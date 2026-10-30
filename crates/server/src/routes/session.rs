//! Logging in and out, and who the session belongs to.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::{self, Session};
use crate::error::{ApiError, ApiResult, Problem};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(log_in))
        .routes(routes!(log_out))
        .routes(routes!(me))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LogIn {
    email: String,
    password: String,
    /// The 6-digit code the authenticator app shows now.
    code: String,
}

#[derive(Serialize, ToSchema)]
pub struct NewSession {
    /// Also set as the `coinroost_session` cookie; clients without cookies send it as a bearer token.
    token: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

impl NewSession {
    pub fn new(token: String, expires_at: OffsetDateTime) -> NewSession {
        NewSession { token, expires_at }
    }
}

/// Starts a session. Every failure answers 401 the same way, whatever was wrong.
#[utoipa::path(post, path = "/api/v1/sessions", tag = "session", security(()), request_body = LogIn,
    responses(
        (status = CREATED, description = "Logged in; the token is also set as a cookie", body = NewSession),
        (status = UNAUTHORIZED, description = "Not logged in", body = Problem)))]
async fn log_in(State(state): State<AppState>, Json(body): Json<LogIn>) -> ApiResult<Response> {
    let (token, expires_at) = auth::login(&state.db, &body.email, &body.password, &body.code).await?;
    let cookie = auth::session_cookie(&token);
    let body = Json(NewSession::new(token, expires_at));
    Ok((StatusCode::CREATED, [(header::SET_COOKIE, cookie)], body).into_response())
}

/// Ends the current session.
#[utoipa::path(delete, path = "/api/v1/sessions/current", tag = "session",
    responses(
        (status = NO_CONTENT, description = "Logged out"),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn log_out(
    State(state): State<AppState>,
    _session: Session,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let token = auth::session_token(&headers).ok_or(ApiError::Unauthorized)?;
    sqlx::query("delete from sessions where token_hash = $1")
        .bind(auth::token_hash(&token))
        .execute(&state.db)
        .await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::SET_COOKIE, auth::cleared_cookie())],
    )
        .into_response())
}

#[derive(Serialize, ToSchema)]
pub struct Me {
    user_id: Uuid,
    household_id: Uuid,
    email: String,
}

/// The user and household of the current session.
#[utoipa::path(get, path = "/api/v1/me", tag = "session",
    responses(
        (status = OK, description = "The session's user", body = Me),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn me(State(state): State<AppState>, session: Session) -> ApiResult<Json<Me>> {
    let (email,): (String,) = sqlx::query_as("select email from users where id = $1")
        .bind(session.user_id)
        .fetch_one(&state.db)
        .await?;
    Ok(Json(Me {
        user_id: session.user_id,
        household_id: session.household_id,
        email,
    }))
}
