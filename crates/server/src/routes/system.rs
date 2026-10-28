//! Liveness of the server and its database.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::error::ApiResult;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(health))
}

#[derive(Serialize, ToSchema)]
pub struct Health {
    /// `ok` when the database answers.
    status: &'static str,
}

/// Whether the server and its database answer. Needs no session.
#[utoipa::path(get, path = "/api/v1/health", tag = "system", security(()),
    responses((status = OK, description = "The server and its database answer", body = Health)))]
async fn health(State(state): State<AppState>) -> ApiResult<Json<Health>> {
    sqlx::query("select 1").execute(&state.db).await?;
    Ok(Json(Health { status: "ok" }))
}
