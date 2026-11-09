//! Feedback told from the app (see [`crate::feedback`]): sending a report, the household's reports, one report, its
//! screenshots, and marking it resolved.

use axum::Json;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};
use crate::feedback::{self, Context, Kind, MAX_BODY_BYTES, NewFeedback};
use crate::validate;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(send_feedback, list_feedback))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .routes(routes!(get_feedback, resolve_feedback))
        .routes(routes!(shot_image))
}

#[derive(Serialize, Deserialize, ToSchema, sqlx::FromRow)]
pub struct ShotInfo {
    /// Its place in the report, from 0, in the order it was taken.
    position: i16,
    note: String,
    /// Read from the image's bytes: `image/jpeg` or `image/png`.
    media_type: String,
    /// The image's size in bytes.
    bytes: i32,
    /// Where the image is: `GET` it with the same session.
    #[serde(default)]
    url: String,
}

#[derive(Serialize, ToSchema)]
pub struct Report {
    id: Uuid,
    kind: Kind,
    message: String,
    context: Context,
    /// The email of who sent it, while that user exists.
    author: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    /// When it was marked resolved; null while it is open.
    #[serde(with = "time::serde::rfc3339::option")]
    resolved_at: Option<OffsetDateTime>,
    shots: Vec<ShotInfo>,
}

#[derive(Serialize, ToSchema)]
pub struct Reports {
    /// Newest first.
    items: Vec<Report>,
}

#[derive(Clone, Copy, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportState {
    Open,
    Resolved,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReportFilter {
    /// Only the open or only the resolved ones; both when absent.
    #[param(inline)]
    state: Option<ReportState>,
    /// At most this many, newest first (50 when absent, 200 at most).
    limit: Option<i64>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    /// True marks it resolved; false opens it again.
    resolved: bool,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: Uuid,
    kind: Kind,
    message: String,
    context: sqlx::types::Json<Context>,
    author: Option<String>,
    created_at: OffsetDateTime,
    resolved_at: Option<OffsetDateTime>,
    shots: sqlx::types::Json<Vec<ShotInfo>>,
}

impl From<Row> for Report {
    fn from(row: Row) -> Report {
        let shots = row
            .shots
            .0
            .into_iter()
            .map(|s| ShotInfo {
                url: format!("/api/v1/feedback/{}/shots/{}", row.id, s.position),
                ..s
            })
            .collect();
        Report {
            id: row.id,
            kind: row.kind,
            message: row.message,
            context: row.context.0,
            author: row.author,
            created_at: row.created_at,
            resolved_at: row.resolved_at,
            shots,
        }
    }
}

/// The household's reports, or one of them (`only`), newest first.
async fn reports(
    db: &sqlx::PgPool,
    household_id: Uuid,
    only: Option<Uuid>,
    state: Option<ReportState>,
    limit: i64,
) -> ApiResult<Vec<Report>> {
    let state = state.map(|s| matches!(s, ReportState::Open));
    let rows: Vec<Row> = sqlx::query_as(
        "select f.id, f.kind, f.message, f.context, u.email as author, f.created_at, f.resolved_at,
                coalesce((select json_agg(json_build_object('position', s.position, 'note', s.note,
                                 'media_type', s.media_type, 'bytes', length(s.image)) order by s.position)
                          from feedback_shots s where s.feedback_id = f.id), '[]'::json) as shots
         from feedback f left join users u on u.id = f.author_id
         where f.household_id = $1 and ($2::uuid is null or f.id = $2)
           and ($3::boolean is null or $3 = (f.resolved_at is null))
         order by f.created_at desc, f.id limit $4",
    )
    .bind(household_id)
    .bind(only)
    .bind(state)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Report::from).collect())
}

async fn one(db: &sqlx::PgPool, household_id: Uuid, id: Uuid) -> ApiResult<Report> {
    reports(db, household_id, Some(id), None, 1)
        .await?
        .pop()
        .ok_or(ApiError::NotFound)
}

/// Keeps a problem or an idea with its screenshots, each with its note. Sending the same `report_id` again answers
/// the report already kept (200) and keeps nothing more.
#[utoipa::path(post, path = "/api/v1/feedback", tag = "feedback", request_body = NewFeedback,
    responses(
        (status = CREATED, description = "Kept", body = Report),
        (status = OK, description = "This draft was already sent: the report kept then", body = Report),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = FORBIDDEN, description = "A read-only token", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "Empty, over a cap, or an image that is not a JPEG or a PNG",
            body = Problem)))]
async fn send_feedback(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewFeedback>,
) -> ApiResult<Response> {
    let checked = feedback::check(&body)?;
    let shots = checked.shots.len();
    let (id, new) = feedback::store(&state.db, session.household_id, session.user_id, &body, checked).await?;
    if new {
        // A line for whoever watches the server's log; never the message or the images.
        eprintln!("feedback {id}: {:?} with {shots} screenshot(s)", body.kind);
    }
    let status = if new { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(one(&state.db, session.household_id, id).await?)).into_response())
}

/// The household's reports, newest first, with each screenshot's note and where its image is.
#[utoipa::path(get, path = "/api/v1/feedback", tag = "feedback", params(ReportFilter),
    responses(
        (status = OK, description = "The reports, newest first", body = Reports),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The limit is not valid", body = Problem)))]
async fn list_feedback(
    State(state): State<AppState>,
    session: Session,
    Query(filter): Query<ReportFilter>,
) -> ApiResult<Json<Reports>> {
    let limit = validate::limit(filter.limit.unwrap_or(50), 200)?;
    let items = reports(&state.db, session.household_id, None, filter.state, limit).await?;
    Ok(Json(Reports { items }))
}

/// One report.
#[utoipa::path(get, path = "/api/v1/feedback/{feedback_id}", tag = "feedback",
    params(("feedback_id" = Uuid, Path, description = "The report")),
    responses(
        (status = OK, description = "The report", body = Report),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such report in the household", body = Problem)))]
async fn get_feedback(
    State(state): State<AppState>,
    session: Session,
    Path(feedback_id): Path<Uuid>,
) -> ApiResult<Json<Report>> {
    one(&state.db, session.household_id, feedback_id).await.map(Json)
}

/// Marks a report resolved, or open again.
#[utoipa::path(patch, path = "/api/v1/feedback/{feedback_id}", tag = "feedback", request_body = Resolution,
    params(("feedback_id" = Uuid, Path, description = "The report")),
    responses(
        (status = OK, description = "The report as it is now", body = Report),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = FORBIDDEN, description = "A read-only token", body = Problem),
        (status = NOT_FOUND, description = "No such report in the household", body = Problem)))]
async fn resolve_feedback(
    State(state): State<AppState>,
    session: Session,
    Path(feedback_id): Path<Uuid>,
    Json(body): Json<Resolution>,
) -> ApiResult<Json<Report>> {
    if !feedback::resolve(&state.db, session.household_id, feedback_id, body.resolved).await? {
        return Err(ApiError::NotFound);
    }
    one(&state.db, session.household_id, feedback_id).await.map(Json)
}

/// A screenshot's image, with the type read from its bytes when it was kept.
#[utoipa::path(get, path = "/api/v1/feedback/{feedback_id}/shots/{position}", tag = "feedback",
    params(("feedback_id" = Uuid, Path, description = "The report"),
           ("position" = i16, Path, description = "The screenshot's place, from 0")),
    responses(
        (status = OK, description = "The image", content((Vec<u8> = "image/jpeg"), (Vec<u8> = "image/png"))),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such report or screenshot in the household", body = Problem)))]
async fn shot_image(
    State(state): State<AppState>,
    session: Session,
    Path((feedback_id, position)): Path<(Uuid, i16)>,
) -> ApiResult<Response> {
    let (media_type, bytes) = feedback::image(&state.db, session.household_id, feedback_id, position).await?;
    // It never changes, and it is the household's own: the browser may keep it, a shared cache may not.
    let headers = [
        (header::CONTENT_TYPE, media_type),
        (
            header::CACHE_CONTROL,
            "private, max-age=31536000, immutable".to_owned(),
        ),
    ];
    Ok((headers, bytes).into_response())
}
