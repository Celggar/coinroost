//! The first account: on a fresh install, whoever reaches the app first makes the owner account from the web. These
//! need no session; what they hand out is an ordinary setup link (see [`crate::invites`]).

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Serialize;
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::error::{ApiError, ApiResult, Problem};
use crate::invites;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(state, start))
}

#[derive(Serialize, ToSchema)]
pub struct FirstAccountState {
    /// True while the app has no users and its deployment lets the first account be made from the web.
    open: bool,
}

/// Whether the first account can be made from the web: the deployment allows it and the app has no users yet.
#[utoipa::path(get, path = "/api/v1/first-account", tag = "invites", security(()),
    responses((status = OK, description = "Whether the first account can still be made", body = FirstAccountState)))]
async fn state(State(state): State<AppState>) -> ApiResult<Json<FirstAccountState>> {
    Ok(Json(FirstAccountState {
        open: state.first_account && invites::first_account_open(&state.db).await?,
    }))
}

#[derive(Serialize, ToSchema)]
pub struct FirstAccountLink {
    /// A setup link's token, used with the `/api/v1/invites/…` operations like any other.
    token: String,
    #[serde(with = "time::serde::rfc3339")]
    expires_at: OffsetDateTime,
}

/// A setup link for the first account, in a new household. It stops working as soon as any user exists.
#[utoipa::path(post, path = "/api/v1/first-account", tag = "invites", security(()),
    responses(
        (status = CREATED, description = "A setup link for the first account", body = FirstAccountLink),
        (status = FORBIDDEN, description = "This deployment makes its first account from the console", body = Problem),
        (status = CONFLICT, description = "The app already has an account; new people need a link from it",
            body = Problem)))]
async fn start(State(state): State<AppState>) -> ApiResult<(StatusCode, Json<FirstAccountLink>)> {
    if !state.first_account {
        return Err(ApiError::Forbidden(
            "this deployment makes its first account from the console: coinroost-server invite add"
                .to_owned(),
        ));
    }
    let link = invites::create_first(&state.db).await?;
    Ok((
        StatusCode::CREATED,
        Json(FirstAccountLink {
            token: link.token,
            expires_at: link.expires_at,
        }),
    ))
}
