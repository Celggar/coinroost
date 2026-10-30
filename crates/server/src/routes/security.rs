//! The caller's own second step: their recovery codes, and the new authenticator a login with one of them leads to
//! (see [`crate::recovery`]).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::{AnySession, Session};
use crate::error::{ApiResult, Problem};
use crate::recovery;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(recovery_status, regenerate))
        .routes(routes!(new_authenticator))
        .routes(routes!(confirm_authenticator))
}

#[derive(Serialize, ToSchema)]
pub struct RecoveryStatus {
    /// How many unused codes are left (out of `size`).
    remaining: i64,
    /// How many codes a set has.
    size: i64,
    /// When the set was made; null when the user has none.
    #[serde(with = "time::serde::rfc3339::option")]
    created_at: Option<OffsetDateTime>,
}

/// How many recovery codes the caller has left. The codes themselves are shown only once, when they are made.
#[utoipa::path(get, path = "/api/v1/me/recovery-codes", tag = "security",
    responses(
        (status = OK, description = "The caller's set", body = RecoveryStatus),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn recovery_status(State(state): State<AppState>, session: Session) -> ApiResult<Json<RecoveryStatus>> {
    let (remaining, created_at) = recovery::status(&state.db, session.user_id).await?;
    Ok(Json(RecoveryStatus {
        remaining,
        size: i64::try_from(recovery::SET_SIZE).unwrap_or(10),
        created_at,
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CurrentCode {
    /// The 6-digit code the authenticator app shows now; spaces or dashes between its groups are fine.
    code: String,
}

#[derive(Serialize, ToSchema)]
pub struct RecoveryCodes {
    /// The new codes, `xxxxx-xxxxx`, each good for one login. Shown only in this answer.
    codes: Vec<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

impl RecoveryCodes {
    pub fn new(set: recovery::NewSet) -> RecoveryCodes {
        RecoveryCodes {
            codes: set.codes,
            created_at: set.created_at,
        }
    }
}

/// Makes a new set of recovery codes; the old set stops working. Asks for a current authenticator code first, and a
/// wrong one counts toward the login lockout.
#[utoipa::path(post, path = "/api/v1/me/recovery-codes", tag = "security", request_body = CurrentCode,
    responses(
        (status = CREATED, description = "The new set, shown once", body = RecoveryCodes),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = FORBIDDEN, description = "Locked after too many wrong codes", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The code does not match", body = Problem)))]
async fn regenerate(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<CurrentCode>,
) -> ApiResult<(StatusCode, Json<RecoveryCodes>)> {
    let set = recovery::regenerate(&state.db, session.user_id, &body.code).await?;
    Ok((StatusCode::CREATED, Json(RecoveryCodes::new(set))))
}

#[derive(Serialize, ToSchema)]
pub struct NewAuthenticator {
    /// The `otpauth://` URI an authenticator app enrolls from.
    uri: String,
    /// The same secret in base32, for apps where it is typed in.
    secret: String,
    /// A QR code of `uri`, as an SVG image.
    qr_svg: String,
}

/// The new authenticator to set up after a login with a recovery code: made by the server on the first call and the
/// same on later ones.
#[utoipa::path(post, path = "/api/v1/me/authenticator", tag = "security",
    responses(
        (status = OK, description = "What the authenticator app needs", body = NewAuthenticator),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The caller's authenticator works; nothing to set up", body = Problem)))]
async fn new_authenticator(
    State(state): State<AppState>,
    session: AnySession,
) -> ApiResult<Json<NewAuthenticator>> {
    let enrollment = recovery::pending_authenticator(&state.db, session.user_id).await?;
    Ok(Json(NewAuthenticator {
        uri: enrollment.uri,
        secret: enrollment.secret_base32,
        qr_svg: enrollment.qr_svg,
    }))
}

/// Makes the new authenticator the caller's, given a current code from it; the old one stays retired and the
/// caller's sessions work fully again.
#[utoipa::path(post, path = "/api/v1/me/authenticator/confirm", tag = "security", request_body = CurrentCode,
    responses(
        (status = NO_CONTENT, description = "The new authenticator is the caller's"),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The caller's authenticator works; nothing to set up", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The code does not match, or no new authenticator was made yet",
            body = Problem)))]
async fn confirm_authenticator(
    State(state): State<AppState>,
    session: AnySession,
    Json(body): Json<CurrentCode>,
) -> ApiResult<StatusCode> {
    recovery::confirm_authenticator(&state.db, session.user_id, &body.code).await?;
    Ok(StatusCode::NO_CONTENT)
}
