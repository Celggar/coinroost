//! The tools that answer "where does the money go?" from the API's reports, already summed: a few rows instead of
//! thousands of transactions.

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use serde_json::{Value, json};

use super::Tools;
use super::client::{Named, decimal, outcome, with_query};
use super::read::{AsOf, Period, TopPayees};

fn money(value: &Value, currency: &Value) -> Value {
    json!(decimal(
        value.as_i64().unwrap_or(0),
        currency.as_str().unwrap_or("USD")
    ))
}

fn fortnight(f: &Value) -> Value {
    let half = if f["half"] == 1 { "first" } else { "second" };
    json!(format!(
        "{}-{:02} {half} half",
        f["year"],
        f["month"].as_u64().unwrap_or(0)
    ))
}

fn fortnights(list: &Value) -> Vec<Value> {
    list.as_array().into_iter().flatten().map(fortnight).collect()
}

#[tool_router(router = report_tools, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Start here for \"where does the money go?\": per currency, spending and income in a period \
                       and spending by category. Money moved between the household's own accounts is not spending; \
                       transfer legs still waiting for their other side are shown apart.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn spending_summary(
        &self,
        Parameters(args): Parameters<Period>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let period = [("from", Some(args.from.clone())), ("to", Some(args.to.clone()))];
                let summary = client.get(&with_query("/api/v1/summary", &period)).await?;
                let report = client.get(&with_query("/api/v1/reports/categories", &period)).await?;
                let names = client.names(Named::Category).await?;
                let currencies: Vec<Value> = summary["totals"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|t| {
                        let c = &t["currency"];
                        let by_category: Vec<Value> = report["items"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|i| &i["currency"] == c)
                            .map(|i| {
                                let name = i["category_id"]
                                    .as_str()
                                    .map_or("(no category)", |id| names.get(id).map_or(id, String::as_str));
                                json!({ "category": name, "spending": money(&i["spending"], c), "count": i["count"] })
                            })
                            .collect();
                        let net = t["income"].as_i64().unwrap_or(0) - t["spending"].as_i64().unwrap_or(0);
                        json!({ "currency": c, "spending": money(&t["spending"], c), "income": money(&t["income"], c),
                                "income_minus_spending": money(&json!(net), c),
                                "unpaired_transfer_out": money(&t["unpaired_transfer_out"], c),
                                "unpaired_transfer_in": money(&t["unpaired_transfer_in"], c),
                                "spending_by_category": by_category })
                    })
                    .collect();
                Ok(json!({ "from": args.from, "to": args.to, "currencies": currencies }))
            }
            .await,
        )
    }

    #[tool(
        description = "Spending and income month by month, with each account's latest data and the gaps between \
                       imported statements. A month that looks cheap almost always lacks a statement: check the \
                       gaps before averaging.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn monthly_trend(
        &self,
        Parameters(args): Parameters<Period>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let period = [("from", Some(args.from.clone())), ("to", Some(args.to.clone()))];
                let monthly = client
                    .get(&with_query("/api/v1/reports/monthly", &period))
                    .await?;
                let coverage = client
                    .get(&with_query(
                        "/api/v1/coverage",
                        &[("today", Some(args.to.clone()))],
                    ))
                    .await?;
                let months: Vec<Value> = monthly["months"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|m| {
                        let c = &m["currency"];
                        json!({ "month": m["month"], "currency": c, "spending": money(&m["spending"], c),
                                "income": money(&m["income"], c) })
                    })
                    .collect();
                let accounts: Vec<Value> = coverage["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|a| {
                        let gaps: Vec<Value> = a["gaps"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|g| json!({ "from": g["from"], "to": g["to"], "days": g["days"] }))
                            .collect();
                        json!({ "account": a["name"], "latest_data": a["latest"], "gaps": gaps })
                    })
                    .collect();
                Ok(json!({ "months": months, "coverage": accounts }))
            }
            .await,
        )
    }

    #[tool(
        description = "The payees most spent on in a period, per currency: the drill-down after spending_summary.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn top_payees(
        &self,
        Parameters(args): Parameters<TopPayees>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let limit = args.limit.unwrap_or(15).clamp(1, 50).to_string();
                let uri = with_query(
                    "/api/v1/reports/payees",
                    &[
                        ("from", Some(args.from)),
                        ("to", Some(args.to)),
                        ("limit", Some(limit)),
                    ],
                );
                let report = client.get(&uri).await?;
                let names = client.names(Named::Payee).await?;
                let rows: Vec<Value> = report["items"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|i| {
                        let name = i["payee_id"]
                            .as_str()
                            .map_or("(no payee)", |id| names.get(id).map_or(id, String::as_str));
                        json!({ "payee": name, "currency": i["currency"],
                                "spending": money(&i["spending"], &i["currency"]), "count": i["count"] })
                    })
                    .collect();
                Ok(json!({ "payees": rows }))
            }
            .await,
        )
    }

    #[tool(
        description = "What each employer still owes, fortnight by fortnight. A fortnight due after the account's \
                       latest statement is unknown (no data), never owed.",
        annotations(read_only_hint = true, open_world_hint = false)
    )]
    async fn payroll_status(
        &self,
        Parameters(args): Parameters<AsOf>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let payroll = client.get(&with_query("/api/v1/payroll", &[("as_of", args.as_of)])).await?;
                let employers: Vec<Value> = payroll["employers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|e| {
                        let c = &e["currency"];
                        json!({ "employer": e["name"], "currency": c, "error": e["error"],
                                "owed": fortnights(&e["owed"]), "owed_total": money(&e["owed_total_minor"], c),
                                "no_data_yet": fortnights(&e["unknown"]), "data_until": e["data_until"],
                                "payments_counted": e["paid"],
                                "next_due": if e["next_due"].is_null() { Value::Null } else { fortnight(&e["next_due"]) } })
                    })
                    .collect();
                Ok(json!({ "as_of": payroll["as_of"], "employers": employers }))
            }
            .await,
        )
    }
}
