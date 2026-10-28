//! `/mcp`: the API for an MCP client (an assistant) over Streamable HTTP. Every tool is a thin adapter that sends a
//! real request through the API router with the caller's bearer, so validation and permissions live only in the
//! API: a read-only token may call a write tool and gets the API's 403 back as the tool's error.
//!
//! - Bearer only: a request without `Authorization: Bearer`, or whose token the API does not accept, gets 401. The
//!   session cookie is never read here, so a page open in a browser cannot drive it.
//! - A request with an `Origin` header is refused with 403: MCP clients are not browsers, and this closes DNS
//!   rebinding. The `Host` must be one of the configured hosts; none configured means loopback, never any host.
//! - Stateless: nothing is kept between requests, so a restart or a proxy loses nothing.

mod client;
mod read;
mod reports;
mod rulebook;
mod write;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::never::NeverSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ServerHandler, tool_handler};
use tower::ServiceExt;

use self::client::{Client, Failure};
use crate::error::ApiError;
use crate::{AppState, auth};

const INSTRUCTIONS: &str = "Coinroost keeps one household's accounts and transactions. Read tools answer from the \
ledger. Write tools change it: before calling one, show the person exactly what will change and wait for their \
explicit yes. Amounts are decimal strings in the account's currency, such as \"45.50\". Spending excludes money moved \
between the household's own accounts; a month that looks cheap usually lacks a statement (see monthly_trend).";

#[derive(Clone)]
pub struct Tools {
    api: Router,
    tool_router: ToolRouter<Tools>,
}

impl Tools {
    fn new(api: Router) -> Tools {
        Tools {
            api,
            tool_router: Tools::read_tools()
                + Tools::report_tools()
                + Tools::write_tools()
                + Tools::rule_tools(),
        }
    }

    fn client(&self, parts: &Parts) -> Result<Client<'_>, Failure> {
        Client::new(&self.api, parts)
    }
}

// The macro writes the tool dispatch as async trait functions, some without an await.
#[allow(clippy::unused_async_trait_impl)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Tools {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("coinroost", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

/// The hosts accepted when none are given: the transport reads an empty list as "any host".
const LOOPBACK: [&str; 3] = ["localhost", "127.0.0.1", "::1"];

/// The `/mcp` route over the API of `state`. `hosts` are the `Host` values accepted (a name matches any port;
/// `host:port` only that port); an empty list means loopback.
pub fn router(state: AppState, hosts: &[String]) -> Router {
    let hosts: Vec<String> = if hosts.is_empty() {
        LOOPBACK.map(str::to_owned).to_vec()
    } else {
        hosts.to_vec()
    };
    let api = crate::router(state);
    let tools_api = api.clone();
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_allowed_hosts(hosts.iter().cloned())
        .enforce_origin_validation();
    let service = StreamableHttpService::new(
        move || Ok(Tools::new(tools_api.clone())),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    Router::new()
        .route_service("/mcp", service)
        .layer(middleware::from_fn_with_state(api, gate))
}

/// Lets through only a request without `Origin` whose bearer the API accepts.
async fn gate(State(api): State<Router>, request: Request, next: Next) -> Response {
    if request.headers().contains_key(header::ORIGIN) {
        return ApiError::Forbidden("a browser may not call /mcp: use an MCP client with a token".to_owned())
            .into_response();
    }
    let Some(token) = auth::bearer(request.headers()) else {
        return ApiError::Unauthorized.into_response();
    };
    let Ok(me) = Request::builder()
        .uri("/api/v1/me")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
    else {
        return ApiError::Unauthorized.into_response();
    };
    match api.oneshot(me).await {
        Ok(answer) if answer.status() == StatusCode::OK => next.run(request).await,
        _ => ApiError::Unauthorized.into_response(),
    }
}
