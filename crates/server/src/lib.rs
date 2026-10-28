//! The Coinroost API server: one household's accounts, transactions, imports and the rules that sort them, on
//! PostgreSQL. The API is the only place where validation and permissions live; every route but the health check
//! and the login needs a session, and every query is scoped to the caller's household.

pub mod auth;
pub mod error;
mod routes;
mod validate;

use axum::Router;
use sqlx::PgPool;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_axum::router::OpenApiRouter;

/// The schema, embedded at build time from `migrations/`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}

#[derive(OpenApi)]
#[openapi(
    info(title = "Coinroost", description = "Personal finance for one household: accounts, transactions, imports."),
    modifiers(&SecuritySchemes),
    security(("session_cookie" = []), ("bearer" = []))
)]
struct ApiDoc;

struct SecuritySchemes;

impl utoipa::Modify for SecuritySchemes {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "session_cookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new(auth::SESSION_COOKIE))),
        );
        components.add_security_scheme(
            "bearer",
            SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
        );
    }
}

fn api() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .merge(routes::system::router())
        .merge(routes::session::router())
        .merge(routes::accounts::router())
        .merge(routes::catalog::router())
        .merge(routes::rules::router())
        .merge(routes::transactions::router())
        .merge(routes::imports::router())
}

/// The HTTP application.
pub fn router(state: AppState) -> Router {
    api().with_state(state).split_for_parts().0
}

/// The API contract (`OpenAPI` 3.1) of [`router`], generated from the same route table.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api().split_for_parts().1
}
