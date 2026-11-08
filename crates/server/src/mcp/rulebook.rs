//! The tools that keep the rules: list them a page at a time, add, reorder, and apply them to existing transactions
//! after a preview. Like every tool, they go through the API with the caller's token.

use std::collections::HashMap;

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
use super::client::{Named, decimal, limit, minor_of, optional, outcome, with_query};
use super::read::name_of;

#[derive(Deserialize, JsonSchema)]
pub struct RuleFilter {
    /// Only rules whose name, or the text they look for, contains this text, upper or lower case alike.
    text: Option<String>,
    /// At most this many, first one first (default 25, at most 50).
    limit: Option<i64>,
    /// The `next_cursor` of the previous answer, with the same text: the rules after it.
    cursor: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct NewRule {
    name: String,
    /// Text the description contains (upper or lower case alike; accents and spaces count).
    text: Option<String>,
    /// out (spending) or in (income); either when absent.
    direction: Option<String>,
    /// Only this account (name or id).
    account: Option<String>,
    /// Smallest amount matched, a decimal such as "25.00".
    amount_min: Option<String>,
    /// Largest amount matched, a decimal.
    amount_max: Option<String>,
    /// The category it sets (name or id).
    category: Option<String>,
    /// The payee it sets (name or id).
    payee: Option<String>,
    /// Name the payee from the text after the matched text (for payment channels).
    payee_from_text: Option<bool>,
}

#[derive(Deserialize, JsonSchema)]
pub struct MoveRule {
    /// The rule (name or id).
    rule: String,
    /// Its new position; 1 is the first rule looked at.
    position: i64,
}

#[derive(Deserialize, JsonSchema)]
pub struct Apply {
    /// The tag `preview_rules` returned: the changes it listed are the ones applied.
    tag: String,
}

/// A rule as a short row with names instead of ids. Amounts follow the rule's account currency, as `create_rule`
/// reads them; two decimals without an account.
fn rule_row(
    r: &Value,
    accounts: &[Value],
    categories: &HashMap<String, String>,
    payees: &HashMap<String, String>,
) -> Value {
    let when = &r["when"];
    let account = accounts.iter().find(|a| a["id"] == when["account_id"]);
    let currency = account.and_then(|a| a["currency"].as_str()).unwrap_or("USD");
    let amount = |v: &Value| v.as_i64().map(|m| decimal(m, currency));
    json!({ "id": r["id"], "position": r["position"], "name": r["name"], "enabled": r["enabled"],
            "when": { "text": when["description_contains"], "direction": when["direction"],
                      "account": account.map_or(json!(""), |a| a["name"].clone()),
                      "amount_min": amount(&when["amount_min"]), "amount_max": amount(&when["amount_max"]) },
            "then": { "category": name_of(categories, &r["then"]["category_id"], ""),
                      "payee": name_of(payees, &r["then"]["payee_id"], ""),
                      "payee_from_text": r["then"]["payee_from_text"] } })
}

#[tool_router(router = rule_tools, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "List the rules in order, a page at a time. The first enabled rule that matches a \
                       transaction sets its category and payee; amounts in rules are decimal strings. `text` finds \
                       rules by name or by the text they look for. `total` counts every match; while \
                       `next_cursor` is not null more follow: pass it as `cursor` for the next page.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn list_rules(
        &self,
        Parameters(args): Parameters<RuleFilter>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let (categories, payees, accounts) = (
                    client.names(Named::Category).await?,
                    client.names(Named::Payee).await?,
                    client.items(Named::Account).await?,
                );
                let uri = with_query(
                    "/api/v1/rules",
                    &[
                        ("q", args.text),
                        ("limit", Some(limit(args.limit, 25))),
                        ("cursor", args.cursor),
                    ],
                );
                let list = client.get(&uri).await?;
                let rows: Vec<Value> = list["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|r| rule_row(r, &accounts, &categories, &payees))
                    .collect();
                Ok(json!({ "rules": rows, "total": list["total"], "next_cursor": list["next_cursor"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "Add a rule at the end of the list. It sorts files imported from now on; existing \
                       transactions change only through preview_rules and apply_rules. WRITES: confirm first.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_rule(
        &self,
        Parameters(args): Parameters<NewRule>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let account = match args.account.as_deref() {
                    Some(given) => Some(client.resolve(Named::Account, given).await?),
                    None => None,
                };
                // Rule amounts follow the account's currency; without one, two decimals.
                let digits = account
                    .as_ref()
                    .and_then(|a| a["currency"].as_str())
                    .and_then(crate::validate::minor_digits)
                    .unwrap_or(2);
                let amount = |text: Option<&str>| text.map(|t| minor_of(t, digits)).transpose();
                let body = json!({
                    "name": args.name,
                    "when": { "description_contains": args.text, "direction": args.direction,
                              "account_id": account.as_ref().map(|a| a["id"].clone()),
                              "amount_min": amount(args.amount_min.as_deref())?,
                              "amount_max": amount(args.amount_max.as_deref())? },
                    "then": { "category_id": optional(&client, Named::Category, args.category.as_deref()).await?,
                              "payee_id": optional(&client, Named::Payee, args.payee.as_deref()).await?,
                              "payee_from_text": args.payee_from_text.unwrap_or(false) } });
                let made = client.send(Method::POST, "/api/v1/rules", Some(&body), None).await?;
                Ok(json!({ "created": made["id"], "name": made["name"], "position": made["position"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "Move a rule to another position; the rules between shift by one. WRITES: confirm first.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn move_rule(
        &self,
        Parameters(args): Parameters<MoveRule>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let rule = client.resolve_id(Named::Rule, &args.rule).await?;
                let body = json!({ "position": args.position });
                let moved = client
                    .send(Method::PATCH, &format!("/api/v1/rules/{rule}"), Some(&body), None)
                    .await?;
                Ok(json!({ "moved": moved["id"], "name": moved["name"], "position": moved["position"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "What applying the rules to existing transactions would change, without changing anything. \
                       A category or payee the person chose is never changed. Returns a tag for apply_rules.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn preview_rules(&self, Extension(parts): Extension<Parts>) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let preview = client.send(Method::POST, "/api/v1/rules/preview", None, None).await?;
                let (categories, payees) =
                    (client.names(Named::Category).await?, client.names(Named::Payee).await?);
                let name = |names: &HashMap<String, String>, id: &Value| {
                    id.as_str().map(|id| names.get(id).cloned().unwrap_or_else(|| id.to_owned()))
                };
                let changes: Vec<Value> = preview["changes"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|c| {
                        let currency = c["currency"].as_str().unwrap_or("USD");
                        json!({ "transaction": c["transaction_id"], "date": c["date"], "description": c["description"],
                                "amount": decimal(c["amount_minor"].as_i64().unwrap_or(0), currency),
                                "category": [name(&categories, &c["before"]["category_id"]),
                                             name(&categories, &c["after"]["category_id"])],
                                "payee": [name(&payees, &c["before"]["payee_id"]),
                                          name(&payees, &c["after"]["payee_id"])
                                              .or_else(|| c["after"]["new_payee"].as_str().map(str::to_owned))] })
                    })
                    .collect();
                Ok(json!({ "tag": preview["tag"], "changes": changes }))
            }
            .await,
        )
    }

    #[tool(
        description = "Apply exactly the changes preview_rules listed under its tag; if anything changed since, the \
                       API refuses and a new preview is needed. WRITES: confirm with the person first.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn apply_rules(
        &self,
        Parameters(args): Parameters<Apply>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let done = client
                    .send(Method::POST, "/api/v1/rules/apply", None, Some(&args.tag))
                    .await?;
                Ok(json!({ "applied": done["applied"] }))
            }
            .await,
        )
    }
}
