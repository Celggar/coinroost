//! The tools that only read: listings, searches and the reports that answer "where does the money go?".

use std::collections::HashMap;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::Tools;
use super::client::{Client, Failure, Named, decimal, limit, outcome, with_query};

#[derive(Deserialize, JsonSchema)]
pub struct AccountFilter {
    /// Only accounts of this kind: `checking`, `savings`, `credit_card`, `cash`, `loan` or `investment`.
    kind: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct TransactionFilter {
    /// First date included, YYYY-MM-DD.
    from: Option<String>,
    /// Last date included, YYYY-MM-DD.
    to: Option<String>,
    /// Only this account (name or id).
    account: Option<String>,
    /// Only this category (name or id).
    category: Option<String>,
    /// Only this payee (name or id).
    payee: Option<String>,
    /// Only transactions without a category.
    uncategorized: Option<bool>,
    /// Only transfers between the household's own accounts.
    transfers: Option<bool>,
    /// At most this many, newest first (default 25, at most 50).
    limit: Option<i64>,
    /// The `next_cursor` of the previous answer, with the same filters: the transactions after it.
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Search {
    /// Text the description contains, upper or lower case alike.
    text: String,
    /// First date included, YYYY-MM-DD.
    from: Option<String>,
    /// Last date included, YYYY-MM-DD.
    to: Option<String>,
    /// Only this category (name or id).
    category: Option<String>,
    /// Only this payee (name or id).
    payee: Option<String>,
    /// At most this many, newest first (default 25, at most 50).
    limit: Option<i64>,
    /// The `next_cursor` of the previous answer, with the same filters: the transactions after it.
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Period {
    /// First date included, YYYY-MM-DD.
    pub(super) from: String,
    /// Last date included, YYYY-MM-DD.
    pub(super) to: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct TopPayees {
    /// First date included, YYYY-MM-DD.
    pub(super) from: String,
    /// Last date included, YYYY-MM-DD.
    pub(super) to: String,
    /// How many payees per currency (default 15, at most 50).
    pub(super) limit: Option<i64>,
}

#[derive(Deserialize, JsonSchema)]
pub struct AsOf {
    /// The day to measure at, YYYY-MM-DD; today when absent.
    pub(super) as_of: Option<String>,
}

pub(super) fn name_of(names: &HashMap<String, String>, id: &Value, none: &str) -> Value {
    id.as_str().map_or_else(
        || json!(none),
        |id| json!(names.get(id).map_or(id, String::as_str)),
    )
}

/// Transactions as short rows with names instead of ids, with how many match and where the next page starts.
async fn rows(client: &Client<'_>, list: &Value) -> Result<Value, Failure> {
    let (accounts, categories, payees) = (
        client.names(Named::Account).await?,
        client.names(Named::Category).await?,
        client.names(Named::Payee).await?,
    );
    let rows: Vec<Value> = list["items"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            let legs: Vec<Value> = t["legs"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|l| {
                    let currency = l["currency"].as_str().unwrap_or("USD");
                    json!({ "account": name_of(&accounts, &l["account_id"], "?"),
                            "amount": decimal(l["amount_minor"].as_i64().unwrap_or(0), currency),
                            "currency": currency })
                })
                .collect();
            json!({ "id": t["id"], "date": t["date"], "description": t["description"],
                    "category": name_of(&categories, &t["category_id"], ""),
                    "category_set_by": t["category_set_by"],
                    "payee": name_of(&payees, &t["payee_id"], ""), "legs": legs })
        })
        .collect();
    let sums: Vec<Value> = list["sums"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let currency = s["currency"].as_str().unwrap_or("USD");
            json!({ "currency": currency, "amount": money_of(&s["amount_minor"], currency) })
        })
        .collect();
    let (total, next_cursor) = (&list["total"], &list["next_cursor"]);
    Ok(json!({ "transactions": rows, "total": total, "next_cursor": next_cursor, "sums": sums }))
}

/// The id of the item `given` names, when one is given.
async fn id_of(client: &Client<'_>, kind: Named, given: Option<&str>) -> Result<Option<String>, Failure> {
    match given {
        Some(given) => Ok(Some(client.resolve_id(kind, given).await?)),
        None => Ok(None),
    }
}

fn money_of(value: &Value, currency: &str) -> Value {
    json!(decimal(value.as_i64().unwrap_or(0), currency))
}

#[tool_router(router = read_tools, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "List the household's accounts with their kind, currency and balance (the sum of their \
                       transactions, as a decimal string).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_accounts(
        &self,
        Parameters(args): Parameters<AccountFilter>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let items = self.client(&parts)?.items(Named::Account).await?;
                let rows: Vec<Value> = items
                    .iter()
                    .filter(|a| args.kind.as_deref().is_none_or(|k| a["kind"] == k))
                    .map(|a| {
                        let currency = a["currency"].as_str().unwrap_or("USD");
                        json!({ "id": a["id"], "name": a["name"], "kind": a["kind"], "currency": currency,
                                "balance": money_of(&a["balance_minor"], currency) })
                    })
                    .collect();
                Ok(json!({ "accounts": rows }))
            }
            .await,
        )
    }

    #[tool(
        description = "List the categories with their kind: expense, income, or transfer (money moved between \
                       the household's own accounts).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_categories(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let items = self.client(&parts)?.items(Named::Category).await?;
                let rows: Vec<Value> = items
                    .iter()
                    .map(|c| json!({ "id": c["id"], "name": c["name"], "kind": c["kind"] }))
                    .collect();
                Ok(json!({ "categories": rows }))
            }
            .await,
        )
    }

    #[tool(
        description = "List transactions, newest first, with names for accounts, categories and payees. A \
                       transfer has two legs; spending is a negative amount. `total` counts every match; while \
                       `next_cursor` is not null more follow: pass it as `cursor`. `sums` is what every match adds \
                       up to per currency, all pages included (money moved between own accounts adds nothing): \
                       for \"how much\", filter and read `sums` instead of paging.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_transactions(
        &self,
        Parameters(args): Parameters<TransactionFilter>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let (account, category, payee) = (
                    id_of(&client, Named::Account, args.account.as_deref()).await?,
                    id_of(&client, Named::Category, args.category.as_deref()).await?,
                    id_of(&client, Named::Payee, args.payee.as_deref()).await?,
                );
                let uri = with_query(
                    "/api/v1/transactions",
                    &[
                        ("from", args.from),
                        ("to", args.to),
                        ("account_id", account),
                        ("category_id", category),
                        ("payee_id", payee),
                        ("uncategorized", args.uncategorized.map(|b| b.to_string())),
                        ("transfers", args.transfers.map(|b| b.to_string())),
                        ("limit", Some(limit(args.limit, 25))),
                        ("cursor", args.cursor),
                    ],
                );
                rows(&client, &client.get(&uri).await?).await
            }
            .await,
        )
    }

    #[tool(
        description = "Find transactions whose description contains a text, upper or lower case alike; \
                       use it to find what to categorize or delete. `total` counts every match; while \
                       `next_cursor` is not null more follow: pass it as `cursor`. `sums` is what every match adds \
                       up to per currency, all pages included (money moved between own accounts adds nothing).",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn search_transactions(
        &self,
        Parameters(args): Parameters<Search>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let (category, payee) = (
                    id_of(&client, Named::Category, args.category.as_deref()).await?,
                    id_of(&client, Named::Payee, args.payee.as_deref()).await?,
                );
                let uri = with_query(
                    "/api/v1/transactions",
                    &[
                        ("q", Some(args.text)),
                        ("from", args.from),
                        ("to", args.to),
                        ("category_id", category),
                        ("payee_id", payee),
                        ("limit", Some(limit(args.limit, 25))),
                        ("cursor", args.cursor),
                    ],
                );
                rows(&client, &client.get(&uri).await?).await
            }
            .await,
        )
    }

    #[tool(
        description = "The server's version and who the token acts for.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn about(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let (health, me) = (client.get("/api/v1/health").await?, client.get("/api/v1/me").await?);
                Ok(json!({ "version": health["version"], "email": me["email"], "household_id": me["household_id"] }))
            }
            .await,
        )
    }
}
