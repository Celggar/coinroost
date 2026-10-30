//! Single-use setup links: how a person gets their user from the web, with no open sign-up.
//!
//! - `coinroost-server invite add` makes a link `…/setup#<token>`. The token is 256 random bits and lives only in the
//!   link's fragment and in request bodies (never a path or query, which proxies log); only its SHA-256 is stored.
//! - A link works for [`INVITE_HOURS`] and once. Opening it, the server generates the authenticator secret and keeps
//!   it with the link; the browser only shows it. Acceptance needs a code from that secret, the email and a password,
//!   and in one transaction marks the link used, creates the user (in the link's household, or a new one), makes
//!   their recovery codes (NIST SP 800-63B-4: issued at enrollment) and opens a session.
//! - A wrong code leaves the link usable; after [`MAX_ATTEMPTS`] wrong codes it stops working. A used, expired,
//!   exhausted or unknown link gets the same answer.
//! - The first account: while there are no users, anyone who reaches the app can get a link for a new household
//!   ([`create_first`]), the way a fresh Firefly III lets its first visitor register. Once any user exists those
//!   links stop working; accepting one checks that under [`auth::USERS_LOCK`], so two at once make one owner.

use std::fmt::Write as _;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use qrcodegen::{QrCode, QrCodeEcc};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth::{self, Enrolled};
use crate::error::{ApiError, ApiResult};
use crate::recovery;

pub const INVITE_HOURS: i64 = 1;
pub const MAX_ATTEMPTS: i32 = 5;

/// A link just made: the token is shown once, here.
pub struct NewInvite {
    pub token: String,
    pub expires_at: OffsetDateTime,
}

/// Makes a setup link for a new user of `household`, or of a new household when none is given.
pub async fn create(db: &PgPool, household: Option<Uuid>) -> ApiResult<NewInvite> {
    let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let expires_at = OffsetDateTime::now_utc() + time::Duration::hours(INVITE_HOURS);
    sqlx::query("insert into invites (token_hash, household_id, expires_at) values ($1, $2, $3)")
        .bind(auth::token_hash(&token))
        .bind(household)
        .bind(expires_at)
        .execute(db)
        .await?;
    Ok(NewInvite { token, expires_at })
}

/// Whether the app has no users yet, so its first account can be made from the web ([`create_first`]).
pub async fn first_account_open(db: &PgPool) -> ApiResult<bool> {
    Ok(sqlx::query_scalar("select not exists (select 1 from users)")
        .fetch_one(db)
        .await?)
}

/// Makes a link for the app's first account, in a new household; refused once any user exists.
pub async fn create_first(db: &PgPool) -> ApiResult<NewInvite> {
    let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let expires_at = OffsetDateTime::now_utc() + time::Duration::hours(INVITE_HOURS);
    let made = sqlx::query(
        "insert into invites (token_hash, expires_at, first_account)
         select $1, $2, true where not exists (select 1 from users)",
    )
    .bind(auth::token_hash(&token))
    .bind(expires_at)
    .execute(db)
    .await?;
    if made.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "this app already has an account; ask its owner for a setup link".to_owned(),
        ));
    }
    Ok(NewInvite { token, expires_at })
}

/// A link that still works.
#[derive(sqlx::FromRow)]
pub struct Live {
    pub id: Uuid,
    pub household_id: Option<Uuid>,
    pub first_account: bool,
    pub totp_secret: Option<Vec<u8>>,
    pub expires_at: OffsetDateTime,
}

/// The link for a token if it still works: not used, not expired, not exhausted, and for a first-account link, no
/// user yet. Anything else is `NotFound`.
pub async fn live(db: &PgPool, token: &str) -> ApiResult<Live> {
    sqlx::query_as::<_, Live>(
        "select id, household_id, first_account, totp_secret, expires_at from invites
         where token_hash = $1 and used_at is null and expires_at > now() and failed_attempts < $2
           and (not first_account or not exists (select 1 from users))",
    )
    .bind(auth::token_hash(token))
    .bind(MAX_ATTEMPTS)
    .fetch_optional(db)
    .await?
    .ok_or(ApiError::NotFound)
}

/// What an authenticator app needs to enroll the link's secret.
pub struct Enrollment {
    pub uri: String,
    pub secret_base32: String,
    pub qr_svg: String,
}

/// The link's authenticator secret, generated on first use and the same afterwards, labeled with `email`.
pub async fn enrollment(db: &PgPool, token: &str, email: &str) -> ApiResult<Enrollment> {
    let invite = live(db, token).await?;
    let (secret,): (Vec<u8>,) = sqlx::query_as(
        "update invites set totp_secret = coalesce(totp_secret, $2) where id = $1 returning totp_secret",
    )
    .bind(invite.id)
    .bind(auth::new_secret())
    .fetch_one(db)
    .await?;
    let uri = auth::enrollment_uri(email.trim(), &secret)?;
    Ok(Enrollment {
        qr_svg: qr_svg(&uri)?,
        secret_base32: auth::secret_base32(&secret)?,
        uri,
    })
}

/// Who accepts a link: the email and password they chose and a code their authenticator shows now.
pub struct Acceptance<'a> {
    pub email: &'a str,
    pub password: &'a str,
    pub code: &'a str,
}

/// A link just used: the new user's session, and their recovery codes, shown once.
pub struct Accepted {
    pub token: String,
    pub expires_at: OffsetDateTime,
    pub recovery: recovery::NewSet,
}

/// Uses the link: creates the user, makes their recovery codes and opens a session. A wrong code counts against the
/// link and leaves it usable; a bad password or a taken email does not count and also leaves it usable.
pub async fn accept(db: &PgPool, token: &str, who: Acceptance<'_>) -> ApiResult<Accepted> {
    let invite = live(db, token).await?;
    if !who.email.trim().get(1..).is_some_and(|rest| rest.contains('@')) {
        return Err(ApiError::Invalid("an email address is needed".to_owned()));
    }
    let secret = invite.totp_secret.ok_or_else(|| {
        ApiError::Invalid("open the link's authenticator step first, then send its code".to_owned())
    })?;
    let password_hash = auth::hash_password(who.password)?;
    let Some(step) = auth::code_step(&secret, who.code)? else {
        sqlx::query("update invites set failed_attempts = failed_attempts + 1 where id = $1")
            .bind(invite.id)
            .execute(db)
            .await?;
        return Err(ApiError::Invalid(
            "the code does not match; wait for the next one and try again".to_owned(),
        ));
    };
    let mut tx = db.begin().await?;
    if invite.first_account {
        still_first(&mut tx).await?;
    }
    // The one statement that uses the link: a second acceptance, even a concurrent one, finds nothing.
    let claimed: Option<(Option<Uuid>,)> = sqlx::query_as(
        "update invites set used_at = now()
         where id = $1 and used_at is null and expires_at > now() and failed_attempts < $2
         returning household_id",
    )
    .bind(invite.id)
    .bind(MAX_ATTEMPTS)
    .fetch_optional(&mut *tx)
    .await?;
    let (household,) = claimed.ok_or(ApiError::NotFound)?;
    let household_id = match household {
        Some(id) => id,
        None => auth::create_household(&mut *tx, "Household").await?,
    };
    let enrolled = Enrolled {
        secret: &secret,
        last_step: step,
    };
    let user_id = auth::insert_user(&mut *tx, household_id, who.email, &password_hash, enrolled).await?;
    sqlx::query("update invites set used_by = $2 where id = $1")
        .bind(invite.id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let recovery = recovery::issue(&mut tx, user_id).await?;
    let (token, expires_at) = auth::new_session(&mut *tx, user_id).await?;
    tx.commit().await?;
    Ok(Accepted {
        token,
        expires_at,
        recovery,
    })
}

/// For a first-account link: `NotFound` once any user exists. Takes [`auth::USERS_LOCK`] and holds it to the commit,
/// as every user insert does, so no other user can appear between this check and the caller's insert.
async fn still_first(tx: &mut PgConnection) -> ApiResult<()> {
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(auth::USERS_LOCK)
        .execute(&mut *tx)
        .await?;
    let someone: bool = sqlx::query_scalar("select exists (select 1 from users)")
        .fetch_one(&mut *tx)
        .await?;
    if someone {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

/// A QR code of `text` as an SVG image, with the quiet zone of four modules the standard asks for.
pub fn qr_svg(text: &str) -> ApiResult<String> {
    const BORDER: i32 = 4;
    let qr = QrCode::encode_text(text, QrCodeEcc::Medium).map_err(|e| ApiError::Internal(e.to_string()))?;
    let size = qr.size() + 2 * BORDER;
    let mut path = String::new();
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                let _ = write!(path, "M{},{}h1v1h-1z", x + BORDER, y + BORDER);
            }
        }
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {size} {size}\" shape-rendering=\"crispEdges\">\
         <rect width=\"100%\" height=\"100%\" fill=\"#fff\"/><path d=\"{path}\" fill=\"#000\"/></svg>"
    ))
}
