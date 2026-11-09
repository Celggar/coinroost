//! Feedback told from the app: a problem or an idea, a message, and up to ten screenshots, each with its own note.
//! Every cap is checked here, whatever the client is, and an image's type is read from its own bytes, never taken
//! from the client. It stays in the household's database: nothing is sent anywhere.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult, Reason};

pub const MAX_SHOTS: usize = 10;
pub const MAX_IMAGE_BYTES: usize = 600_000;
const MAX_NOTE: usize = 500;
const MAX_MESSAGE: usize = 4_000;
/// Ten images at the cap in base64 (8,000,000 characters), and the rest of the report.
pub const MAX_BODY_BYTES: usize = 9 * 1024 * 1024;

/// What the person is telling: something that does not work, or an idea.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum Kind {
    Problem,
    Idea,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scheme {
    Light,
    Dark,
}

/// Where the report was sent from, as the browser said it.
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Context {
    /// The screen's path, with its query: `/accounts`.
    pub path: String,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub color_scheme: Scheme,
    /// The app's language, a BCP 47 tag: `es`.
    pub language: String,
    pub user_agent: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewShot {
    /// The image in base64 (RFC 4648, with padding): a JPEG or a PNG of at most 600,000 bytes.
    pub image: String,
    /// What this screenshot shows, at most 500 characters.
    #[serde(default)]
    pub note: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewFeedback {
    /// Made by the client once per draft: sending the same report again makes no second one.
    pub report_id: Uuid,
    pub kind: Kind,
    /// At most 4,000 characters; it may be empty when there is a screenshot.
    #[serde(default)]
    pub message: String,
    pub context: Context,
    /// At most ten, in the order they were taken.
    #[serde(default)]
    pub shots: Vec<NewShot>,
}

/// One screenshot, checked: its bytes, the type they are, and its note.
pub struct Shot {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
    pub note: String,
}

/// A report that passed every check.
pub struct Checked {
    pub message: String,
    pub shots: Vec<Shot>,
}

fn invalid(code: &'static str, detail: String, at: &str) -> ApiError {
    ApiError::Invalid(Reason::new(code, detail).at(at))
}

/// JPEG starts with FF D8 FF; PNG with its 8-byte signature (RFC 2083). Anything else is not taken.
fn media_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else {
        None
    }
}

fn shot(index: usize, shot: &NewShot) -> ApiResult<Shot> {
    let at = |member: &str| format!("shots.{index}.{member}");
    // Longer than the cap's base64 cannot fit: refused before decoding.
    if shot.image.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
        return Err(invalid(
            "image_too_large",
            format!("an image is at most {MAX_IMAGE_BYTES} bytes"),
            &at("image"),
        ));
    }
    let bytes = STANDARD.decode(shot.image.trim()).map_err(|_| {
        invalid(
            "image_encoding",
            "the image must be base64".to_owned(),
            &at("image"),
        )
    })?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(invalid(
            "image_too_large",
            format!("an image is 1 to {MAX_IMAGE_BYTES} bytes"),
            &at("image"),
        ));
    }
    let media_type = media_type(&bytes).ok_or_else(|| {
        invalid(
            "image_type",
            "an image must be a JPEG or a PNG".to_owned(),
            &at("image"),
        )
    })?;
    let note = shot.note.trim().to_owned();
    if note.chars().count() > MAX_NOTE {
        return Err(invalid(
            "text_length",
            format!("a note is at most {MAX_NOTE} characters"),
            &at("note"),
        ));
    }
    Ok(Shot {
        bytes,
        media_type,
        note,
    })
}

fn context(context: &Context) -> ApiResult<()> {
    let text = |value: &str, max: usize, member: &str| {
        let length = value.chars().count();
        if length == 0 || length > max {
            return Err(invalid(
                "text_length",
                format!("{member} needs 1 to {max} characters"),
                &format!("context.{member}"),
            ));
        }
        Ok(())
    };
    text(&context.path, 512, "path")?;
    if !context.path.starts_with('/') {
        return Err(invalid(
            "path",
            "the path starts with /".to_owned(),
            "context.path",
        ));
    }
    text(&context.language, 35, "language")?;
    text(&context.user_agent, 512, "user_agent")?;
    for (size, member) in [
        (context.viewport_width, "viewport_width"),
        (context.viewport_height, "viewport_height"),
    ] {
        if !(1..=20_000).contains(&size) {
            return Err(invalid(
                "viewport",
                "a viewport side is 1 to 20000 pixels".to_owned(),
                &format!("context.{member}"),
            ));
        }
    }
    Ok(())
}

/// Every cap, in the order a person would fix them: the message, the count, then each screenshot.
pub fn check(report: &NewFeedback) -> ApiResult<Checked> {
    let message = report.message.trim().to_owned();
    if message.chars().count() > MAX_MESSAGE {
        return Err(invalid(
            "text_length",
            format!("the message is at most {MAX_MESSAGE} characters"),
            "message",
        ));
    }
    if message.is_empty() && report.shots.is_empty() {
        return Err(invalid(
            "empty_feedback",
            "write a message or add a screenshot".to_owned(),
            "message",
        ));
    }
    if report.shots.len() > MAX_SHOTS {
        return Err(invalid(
            "too_many_shots",
            format!("at most {MAX_SHOTS} screenshots"),
            "shots",
        ));
    }
    context(&report.context)?;
    let shots = report
        .shots
        .iter()
        .enumerate()
        .map(|(i, s)| shot(i, s))
        .collect::<ApiResult<_>>()?;
    Ok(Checked { message, shots })
}

/// Stores a checked report; the id, and whether it is new (false: this draft was already sent).
pub async fn store(
    db: &PgPool,
    household_id: Uuid,
    author_id: Uuid,
    report: &NewFeedback,
    checked: Checked,
) -> ApiResult<(Uuid, bool)> {
    let mut tx = db.begin().await?;
    let made: Option<Uuid> = sqlx::query_scalar(
        "insert into feedback (household_id, author_id, report_id, kind, message, context)
         values ($1, $2, $3, $4, $5, $6) on conflict (household_id, report_id) do nothing returning id",
    )
    .bind(household_id)
    .bind(author_id)
    .bind(report.report_id)
    .bind(report.kind)
    .bind(&checked.message)
    .bind(Json(&report.context))
    .fetch_optional(&mut *tx)
    .await?;
    let Some(id) = made else {
        let id = sqlx::query_scalar("select id from feedback where household_id = $1 and report_id = $2")
            .bind(household_id)
            .bind(report.report_id)
            .fetch_one(&mut *tx)
            .await?;
        return Ok((id, false));
    };
    for (position, shot) in checked.shots.into_iter().enumerate() {
        sqlx::query("insert into feedback_shots (feedback_id, position, media_type, image, note) values ($1, $2, $3, $4, $5)")
            .bind(id)
            .bind(i16::try_from(position).unwrap_or(i16::MAX))
            .bind(shot.media_type)
            .bind(shot.bytes)
            .bind(shot.note)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok((id, true))
}

/// A screenshot's bytes and type, if the report is the household's.
pub async fn image(db: &PgPool, household_id: Uuid, id: Uuid, position: i16) -> ApiResult<(String, Vec<u8>)> {
    sqlx::query_as(
        "select s.media_type, s.image from feedback_shots s join feedback f on f.id = s.feedback_id
         where f.id = $1 and f.household_id = $2 and s.position = $3",
    )
    .bind(id)
    .bind(household_id)
    .bind(position)
    .fetch_optional(db)
    .await?
    .ok_or(ApiError::NotFound)
}

/// Marks a report resolved now, or open again; false when it is not the household's.
pub async fn resolve(db: &PgPool, household_id: Uuid, id: Uuid, resolved: bool) -> ApiResult<bool> {
    let changed = sqlx::query(
        "update feedback set resolved_at = case when $3 then coalesce(resolved_at, now()) end
         where id = $1 and household_id = $2",
    )
    .bind(id)
    .bind(household_id)
    .bind(resolved)
    .execute(db)
    .await?
    .rows_affected();
    Ok(changed == 1)
}
