//! The Coinroost API server: one household's accounts, transactions, imports and the rules that sort them, on
//! PostgreSQL. The API is the only place where validation and permissions live; every route but the health check
//! and the login needs a session, and every query is scoped to the caller's household. The same server can serve
//! the web app, a static build, from a directory.

pub mod account_rules;
pub mod auth;
pub mod employers;
pub mod error;
pub mod firefly;
mod import;
pub mod invites;
pub mod mcp;
mod pairing;
pub mod recovery;
mod routes;
pub mod rule_set;
mod sorting;
pub mod tokens;
mod validate;

use std::path::Path;

use axum::Router;
use axum::http::{HeaderValue, header};
use axum::routing::any;
use sqlx::PgPool;
use tower::Layer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use utoipa::OpenApi;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa_axum::router::OpenApiRouter;

/// The page the static web build serves for every route of the app (the adapter's single-page fallback).
pub const SHELL: &str = "200.html";

/// The schema, embedded at build time from `migrations/`.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Whether a fresh install lets the first person who reaches it make the owner account from the web
    /// (`COINROOST_FIRST_ACCOUNT=open`). Off by default: an install open to the internet would hand itself to
    /// whoever comes first, so only a deployment that controls who reaches it (an access proxy) turns it on.
    pub first_account: bool,
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
        .merge(routes::security::router())
        .merge(routes::first_account::router())
        .merge(routes::invites::router())
        .merge(routes::accounts::router())
        .merge(routes::catalog::router())
        .merge(routes::rules::router())
        .merge(routes::apply::router())
        .merge(routes::transactions::router())
        .merge(routes::removal::router())
        .merge(routes::imports::router())
        .merge(routes::coverage::router())
        .merge(routes::transfers::router())
        .merge(routes::summary::router())
        .merge(routes::payroll::router())
        .merge(routes::reports::router())
}

/// The HTTP API. A path under `/api/` that is not in the contract answers a problem+json 404.
pub fn router(state: AppState) -> Router {
    api()
        .with_state(state)
        .split_for_parts()
        .0
        .route("/api/{*rest}", any(|| async { error::ApiError::NotFound }))
}

/// The API, and the web app built into `web` when given: its files, and the app shell ([`SHELL`]) for any other
/// path so the app's own routes load. Fingerprinted assets (`_app/immutable/`) are cached for good; the rest is revalidated.
pub fn app(state: AppState, web: Option<&Path>) -> Router {
    let app = router(state);
    let app = match web {
        Some(dir) => {
            let immutable = SetResponseHeaderLayer::overriding(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            )
            .layer(ServeDir::new(dir.join("_app/immutable")));
            let pages = SetResponseHeaderLayer::if_not_present(
                header::CACHE_CONTROL,
                HeaderValue::from_static("no-cache"),
            )
            .layer(ServeDir::new(dir).fallback(ServeFile::new(dir.join(SHELL))));
            app.nest_service("/_app/immutable", immutable)
                .fallback_service(pages)
        }
        None => app,
    };
    app.layer(SetResponseHeaderLayer::overriding(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    ))
    .layer(SetResponseHeaderLayer::overriding(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("DENY"),
    ))
    .layer(SetResponseHeaderLayer::overriding(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    ))
}

/// The API contract (`OpenAPI` 3.1) of [`router`], generated from the same route table.
pub fn openapi() -> utoipa::openapi::OpenApi {
    api().split_for_parts().1
}
