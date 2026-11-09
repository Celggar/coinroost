//! The tools for money moved between the household's own accounts: what waits to be paired and why, pairing it, and
//! splitting a transfer back. Each sends the API's own request, so its checks (one leg each, other sign, other account,
//! same currency) stay the only ones.
use std::collections::{BTreeMap, HashMap};

use axum::http::Method;
use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Tools;
use super::client::{Named, decimal, outcome};
use super::read::name_of;

#[derive(Deserialize, JsonSchema)]
pub struct Waiting {
    /// At most this many pairs and this many unpaired legs, newest first (50 when absent, 200 at most).
    limit: Option<usize>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Ready {
    /// The `tag` `transfers_waiting` gave with the pairs the person agreed to.
    tag: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct Pair {
    /// The two transaction ids, one leg each: the money leaving one account and the same money arriving in another.
    transactions: [String; 2],
}

#[derive(Deserialize, JsonSchema)]
pub struct Transfer {
    /// The transfer's transaction id.
    id: String,
}

/// One leg as a short row: the account's and category's names, the signed amount as a decimal.
fn leg(l: &Value, accounts: &HashMap<String, String>, categories: &HashMap<String, String>) -> Value {
    let currency = l["currency"].as_str().unwrap_or("USD");
    json!({ "transaction": l["transaction_id"], "date": l["date"], "description": l["description"],
            "account": name_of(accounts, &l["account_id"], "?"),
            "amount": decimal(l["amount_minor"].as_i64().unwrap_or(0), currency), "currency": currency,
            "category": name_of(categories, &l["category_id"], "") })
}

#[tool_router(router = transfer_tools, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "What waits to become a transfer between the household's own accounts, over the legs whose \
                       category is a transfer. `pairs` are the ones the app is sure of (pair them with \
                       pair_waiting_transfers and this `tag`). `legs` are left alone, each with why: no_counterpart \
                       (no other side within two days, often a statement not imported yet) or several_candidates \
                       (more than one could be it), and the legs that could be its other side. Reads only.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn transfers_waiting(
        &self,
        Parameters(args): Parameters<Waiting>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let preview = client
                    .send(Method::POST, "/api/v1/transfers/preview", None, None)
                    .await?;
                let (accounts, categories) = (
                    client.names(Named::Account).await?,
                    client.names(Named::Category).await?,
                );
                let row = |l: &Value| leg(l, &accounts, &categories);
                let limit = args.limit.unwrap_or(50).min(200);
                let all_pairs = preview["pairs"].as_array().cloned().unwrap_or_default();
                let unmatched = preview["unmatched"].as_array().cloned().unwrap_or_default();
                let mut by_reason = BTreeMap::<String, usize>::new();
                for u in &unmatched {
                    *by_reason.entry(u["reason"].as_str().unwrap_or("?").to_owned()).or_default() += 1;
                }
                let pairs: Vec<Value> = all_pairs
                    .iter()
                    .rev()
                    .take(limit)
                    .map(|p| {
                        json!({ "outflow": row(&p["outflow"]), "inflow": row(&p["inflow"]), "pass": p["pass"],
                                "drops_manual": p["drops_manual"] })
                    })
                    .collect();
                let legs: Vec<Value> = unmatched
                    .iter()
                    .rev()
                    .take(limit)
                    .map(|u| {
                        let candidates: Vec<Value> = u["candidates"].as_array().into_iter().flatten().map(row).collect();
                        json!({ "leg": row(&u["leg"]), "reason": u["reason"], "candidates": candidates })
                    })
                    .collect();
                Ok(json!({ "pairs_ready": all_pairs.len(), "unpaired": unmatched.len(), "by_reason": by_reason,
                           "tag": preview["tag"], "pairs": pairs, "legs": legs }))
            }
            .await,
        )
    }

    #[tool(
        description = "Pair every pair transfers_waiting listed as ready, exactly those: the `tag` it gave makes the \
                       API refuse if they changed since. WRITES to the ledger: show the person the pairs and wait for \
                       their yes. Balances do not change; split_transfer undoes one.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn pair_waiting_transfers(
        &self,
        Parameters(args): Parameters<Ready>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let done = client
                    .send(Method::POST, "/api/v1/transfers/apply", None, Some(&args.tag))
                    .await?;
                Ok(json!({ "paired": done["paired"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "Pair two one-leg transactions as one transfer between two of the household's own accounts, \
                       whatever their dates and categories: money leaving one account and the same amount arriving in \
                       the other. For a leg transfers_waiting left alone, once its other side is clear (a card number, \
                       an account number in the text, an ACH that lands days later). WRITES to the ledger: show the \
                       person both and wait for their yes. Balances do not change; split_transfer undoes it.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn pair_transfer(
        &self,
        Parameters(args): Parameters<Pair>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let body = json!({ "transaction_ids": args.transactions });
                let made = client
                    .send(Method::POST, "/api/v1/transfers", Some(&body), None)
                    .await?;
                Ok(json!({ "transfer": made["transaction_id"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "Split a transfer back into its two one-leg transactions, to undo a pairing that was wrong. \
                       WRITES to the ledger: show the person the transfer and wait for their yes. Balances do not \
                       change.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn split_transfer(
        &self,
        Parameters(args): Parameters<Transfer>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let uri = format!("/api/v1/transactions/{}/split", args.id);
                let split = client.send(Method::POST, &uri, None, None).await?;
                Ok(json!({ "transactions": split["transaction_ids"] }))
            }
            .await,
        )
    }
}
