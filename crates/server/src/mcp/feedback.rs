//! The tools for what the household told about the app: the reports waiting, one report with each screenshot next to
//! its own note, and marking one resolved. Each sends the API's own request.

use axum::http::Method;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock};
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Tools;
use super::client::{Failure, limit, outcome, with_query};

#[derive(Deserialize, JsonSchema)]
pub struct Listing {
    /// `open` (when absent), `resolved` or `all`.
    state: Option<String>,
    /// At most this many, newest first (20 when absent, 50 at most).
    limit: Option<i64>,
}

#[derive(Deserialize, JsonSchema)]
pub struct One {
    /// The report's id, from `feedback_list`.
    id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct Resolving {
    /// The report's id, from `feedback_list`.
    id: String,
    /// True (when absent) marks it resolved; false opens it again.
    resolved: Option<bool>,
}

/// The report's path, from an id that must be one: never a path of the caller's making.
fn path_of(id: &str) -> Result<String, Failure> {
    let id = uuid::Uuid::parse_str(id.trim()).map_err(|_| Failure(format!("{id} is not a report id")))?;
    Ok(format!("/api/v1/feedback/{id}"))
}

/// A report without the screenshots' addresses, which only the API's own client follows.
fn plain(report: &Value) -> Value {
    let notes: Vec<Value> = report["shots"]
        .as_array()
        .map(|shots| shots.iter().map(|s| s["note"].clone()).collect())
        .unwrap_or_default();
    json!({ "id": report["id"], "kind": report["kind"], "message": report["message"],
            "screen": report["context"]["path"], "context": report["context"], "author": report["author"],
            "created_at": report["created_at"], "resolved_at": report["resolved_at"], "screenshot_notes": notes })
}

#[tool_router(router = feedback_tools, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "What the household told about the app from its feedback panel, newest first: each report's \
                       kind (problem or idea), message, the screen it was sent from, and its screenshots' notes. \
                       Open ones by default. Read one with feedback_get to see its screenshots. Reads only.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn feedback_list(
        &self,
        Parameters(args): Parameters<Listing>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let state = match args.state.as_deref().unwrap_or("open") {
                    "all" => None,
                    state @ ("open" | "resolved") => Some(state.to_owned()),
                    other => return Err(Failure(format!("state is open, resolved or all, not {other}"))),
                };
                let uri = with_query(
                    "/api/v1/feedback",
                    &[("state", state), ("limit", Some(limit(args.limit, 20)))],
                );
                let list = self.client(&parts)?.get(&uri).await?;
                let items: Vec<Value> = list["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(plain)
                    .collect();
                Ok(json!({ "reports": items }))
            }
            .await,
        )
    }

    #[tool(
        description = "One feedback report: its message and context, then each screenshot as an image right after \
                       its own note, in the order they were taken. Reads only.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn feedback_get(
        &self,
        Parameters(args): Parameters<One>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let read = async {
            let client = self.client(&parts)?;
            let report = client.get(&path_of(&args.id)?).await?;
            let mut content = vec![ContentBlock::text(plain(&report).to_string())];
            for (i, shot) in report["shots"].as_array().into_iter().flatten().enumerate() {
                let note = shot["note"].as_str().unwrap_or_default();
                content.push(ContentBlock::text(format!("Screenshot {}: {note}", i + 1)));
                let (kind, bytes) = client.bytes(shot["url"].as_str().unwrap_or_default()).await?;
                content.push(ContentBlock::image(STANDARD.encode(bytes), kind));
            }
            Ok::<_, Failure>(content)
        };
        Ok(match read.await {
            Ok(content) => CallToolResult::success(content),
            Err(Failure(reason)) => CallToolResult::error(vec![ContentBlock::text(reason)]),
        })
    }

    #[tool(
        description = "Mark a feedback report resolved once what it asked is done, or open it again. WRITES the \
                       report's state only; nothing in the ledger changes.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn feedback_resolve(
        &self,
        Parameters(args): Parameters<Resolving>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let body = json!({ "resolved": args.resolved.unwrap_or(true) });
                let uri = path_of(&args.id)?;
                let report = self
                    .client(&parts)?
                    .send(Method::PATCH, &uri, Some(&body), None)
                    .await?;
                Ok(json!({ "id": report["id"], "resolved_at": report["resolved_at"] }))
            }
            .await,
        )
    }
}
