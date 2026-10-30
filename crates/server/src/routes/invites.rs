//! Setup links: checking one, enrolling its authenticator and accepting it. These need no session; the link's
//! token, always in the body, is the credential (see [`crate::invites`]).

use axum::Json;
use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth;
use crate::error::{ApiResult, Problem};
use crate::invites::{self, Acceptance};
use crate::routes::session::NewSession;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(check))
        .routes(routes!(enrollment))
        .routes(routes!(accept))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteToken {
    /// The token from the link's fragment (after `#`).
    token: String,
}

#[derive(Serialize, ToSchema)]
pub struct InviteState {
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
    /// True when accepting it makes a new household; false when it joins an existing one.
    new_household: bool,
    /// True for the app's first account (see `/api/v1/first-account`).
    first_account: bool,
}

/// Whether a setup link still works. A used, expired or unknown link answers 404 the same way.
#[utoipa::path(post, path = "/api/v1/invites/check", tag = "invites", security(()), request_body = InviteToken,
    responses(
        (status = OK, description = "The link works", body = InviteState),
        (status = NOT_FOUND, description = "The link was used, expired or does not exist", body = Problem)))]
async fn check(State(state): State<AppState>, Json(body): Json<InviteToken>) -> ApiResult<Json<InviteState>> {
    let invite = invites::live(&state.db, &body.token).await?;
    Ok(Json(InviteState {
        expires_at: invite.expires_at,
        new_household: invite.household_id.is_none(),
        first_account: invite.first_account,
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EnrollmentRequest {
    token: String,
    /// The email the person will log in with; it only labels the entry in their authenticator app.
    email: String,
}

#[derive(Serialize, ToSchema)]
pub struct InviteEnrollment {
    /// The `otpauth://` URI an authenticator app enrolls from.
    uri: String,
    /// The same secret in base32, for apps where it is typed in.
    secret: String,
    /// A QR code of `uri`, as an SVG image.
    qr_svg: String,
}

/// The link's authenticator secret, made by the server the first time and the same on every later call.
#[utoipa::path(post, path = "/api/v1/invites/enrollment", tag = "invites", security(()),
    request_body = EnrollmentRequest,
    responses(
        (status = OK, description = "What the authenticator app needs", body = InviteEnrollment),
        (status = NOT_FOUND, description = "The link was used, expired or does not exist", body = Problem)))]
async fn enrollment(
    State(state): State<AppState>,
    Json(body): Json<EnrollmentRequest>,
) -> ApiResult<Json<InviteEnrollment>> {
    let enrolled = invites::enrollment(&state.db, &body.token, &body.email).await?;
    Ok(Json(InviteEnrollment {
        uri: enrolled.uri,
        secret: enrolled.secret_base32,
        qr_svg: enrolled.qr_svg,
    }))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AcceptInvite {
    token: String,
    email: String,
    /// At least 12 characters.
    password: String,
    /// The 6-digit code the authenticator app shows now for the link's secret; spaces or dashes between its groups
    /// are fine.
    code: String,
}

/// Uses the link once: creates the user with the email, password and authenticator secret, and logs them in.
#[utoipa::path(post, path = "/api/v1/invites/accept", tag = "invites", security(()), request_body = AcceptInvite,
    responses(
        (status = CREATED, description = "The user exists and is logged in; the token is also set as a cookie",
            body = NewSession),
        (status = NOT_FOUND, description = "The link was used, expired or does not exist", body = Problem),
        (status = CONFLICT, description = "A user with that email already exists", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "Wrong code, short password, no email, or no authenticator \
            step yet; the link still works", body = Problem)))]
async fn accept(State(state): State<AppState>, Json(body): Json<AcceptInvite>) -> ApiResult<Response> {
    let who = Acceptance {
        email: &body.email,
        password: &body.password,
        code: &body.code,
    };
    let (token, expires_at) = invites::accept(&state.db, &body.token, who).await?;
    let cookie = auth::session_cookie(&token);
    let body = Json(NewSession::new(token, expires_at));
    Ok((StatusCode::CREATED, [(header::SET_COOKIE, cookie)], body).into_response())
}
