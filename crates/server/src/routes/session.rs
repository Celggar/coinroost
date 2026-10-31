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
use crate::auth::{self, AnySession, Second, Session};
use crate::error::{ApiError, ApiResult, Problem, Reason};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(log_in))
        .routes(routes!(log_out))
        .routes(routes!(me, change_me))
}

/// The languages the web app has texts for, as BCP 47 primary tags.
const LOCALES: [&str; 2] = ["es", "en"];

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LogIn {
    email: String,
    password: String,
    /// The 6-digit code the authenticator app shows now; spaces or dashes between its groups are fine. Send this or
    /// `recovery_code`, not both.
    #[serde(default)]
    code: Option<String>,
    /// One of the user's recovery codes (`xxxxx-xxxxx`; case, spaces and dashes do not matter), in place of `code`
    /// when the authenticator app is lost. It works once, ends the user's other sessions and retires the
    /// authenticator: the new session may only set up a new one until it is confirmed (see `GET /api/v1/me`).
    #[serde(default)]
    recovery_code: Option<String>,
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

/// Starts a session. Every failure answers 401 the same way, whatever was wrong; a body with both or neither of
/// `code` and `recovery_code` answers 422.
#[utoipa::path(post, path = "/api/v1/sessions", tag = "session", security(()), request_body = LogIn,
    responses(
        (status = CREATED, description = "Logged in; the token is also set as a cookie", body = NewSession),
        (status = UNAUTHORIZED, description = "Not logged in", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "Both or neither of the two second steps", body = Problem)))]
async fn log_in(State(state): State<AppState>, Json(body): Json<LogIn>) -> ApiResult<Response> {
    let second = match (&body.code, &body.recovery_code) {
        (Some(code), None) => Second::Code(code),
        (None, Some(code)) => Second::Recovery(code),
        _ => {
            return Err(ApiError::invalid(
                "code_or_recovery_code",
                "send the authenticator's `code` or a `recovery_code`, one of the two".to_owned(),
            ));
        }
    };
    let (token, expires_at) = auth::login(&state.db, &body.email, &body.password, second).await?;
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
    _session: AnySession,
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
    /// True after a login with a recovery code, until a new authenticator is confirmed
    /// (`POST /api/v1/me/authenticator`); until then every other route answers 403.
    reenroll_required: bool,
    /// The language the user picked (`es` or `en`), saved for every device; null until they pick one, and then the
    /// browser's language decides.
    locale: Option<String>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MeChange {
    /// The language the app speaks to this user: `es` or `en`.
    locale: String,
}

/// The user and household of the current session; it also answers a session that must set up a new authenticator.
#[utoipa::path(get, path = "/api/v1/me", tag = "session",
    responses(
        (status = OK, description = "The session's user", body = Me),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn me(State(state): State<AppState>, session: AnySession) -> ApiResult<Json<Me>> {
    let (email, locale): (String, Option<String>) =
        sqlx::query_as("select email, locale from users where id = $1")
            .bind(session.user_id)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(Me {
        user_id: session.user_id,
        household_id: session.household_id,
        email,
        reenroll_required: session.reenroll_required,
        locale,
    }))
}

/// Saves the user's language on their account, so it wins over the browser's on every device after login. The API
/// is where it is checked: a language the app has no texts for is refused, whoever calls.
#[utoipa::path(patch, path = "/api/v1/me", tag = "session", request_body = MeChange,
    responses(
        (status = OK, description = "The user, as saved", body = Me),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = FORBIDDEN, description = "A new authenticator must be set up first", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "A language the app has no texts for (`unknown_locale`)",
            body = Problem)))]
async fn change_me(
    State(state): State<AppState>,
    session: Session,
    Json(change): Json<MeChange>,
) -> ApiResult<Json<Me>> {
    if !LOCALES.contains(&change.locale.as_str()) {
        let detail = format!("no texts for {:?}: use es or en", change.locale);
        return Err(ApiError::Invalid(
            Reason::new("unknown_locale", detail).at("locale"),
        ));
    }
    sqlx::query("update users set locale = $2 where id = $1")
        .bind(session.user_id)
        .bind(&change.locale)
        .execute(&state.db)
        .await?;
    let caller = AnySession {
        user_id: session.user_id,
        household_id: session.household_id,
        reenroll_required: false,
    };
    me(State(state), caller).await
}
