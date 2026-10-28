//! The tools that change transactions and categories. Each says to confirm with the person first; the API checks
//! the token's scope and every value, as it does for the web app.

use axum::http::Method;
use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::Tools;
use super::client::{Failure, Named, decimal, optional, outcome, to_minor};

#[derive(Deserialize, JsonSchema)]
pub struct NewTransaction {
    /// The date, YYYY-MM-DD.
    date: String,
    /// spending (money out of `account`), income (money into it) or transfer (from `account` to `to_account`).
    kind: String,
    /// A positive decimal in the account's currency, such as "45.50".
    amount: String,
    /// The account (name or id).
    account: String,
    /// For a transfer: the account the money goes to (name or id).
    to_account: Option<String>,
    description: String,
    /// A category (name or id).
    category: Option<String>,
    /// A payee (name or id).
    payee: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Sorting {
    /// The transaction id.
    id: String,
    /// A category (name or id); an empty text removes it.
    category: Option<String>,
    /// A payee (name or id); an empty text removes it.
    payee: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
pub struct Categorize {
    /// The transaction id.
    id: String,
    /// The category (name or id).
    category: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct Id {
    /// The transaction id.
    id: String,
}

#[derive(Deserialize, JsonSchema)]
pub struct NewCategory {
    name: String,
    /// expense (the default), income, or transfer (money moved between the household's own accounts).
    kind: Option<String>,
}

#[tool_router(router = write_tools, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Add one transaction by hand (a cash purchase, a payment the bank has not shown yet). WRITES \
                       to the ledger: show the person what will be added and wait for their yes.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_transaction(
        &self,
        Parameters(args): Parameters<NewTransaction>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let account = client.resolve(Named::Account, &args.account).await?;
                let currency = account["currency"].as_str().unwrap_or("USD");
                let amount = to_minor(&args.amount, currency)?;
                let leg = |account: &Value, minor: i64| json!({ "account_id": account["id"], "amount_minor": minor });
                let legs = match (args.kind.as_str(), args.to_account.as_deref()) {
                    ("spending", None) => vec![leg(&account, -amount)],
                    ("income", None) => vec![leg(&account, amount)],
                    ("transfer", Some(to)) => {
                        let to = client.resolve(Named::Account, to).await?;
                        vec![leg(&account, -amount), leg(&to, amount)]
                    }
                    _ => {
                        return Err(Failure(
                            "kind is spending or income without to_account, or transfer with it".to_owned(),
                        ));
                    }
                };
                let body = json!({ "date": args.date, "description": args.description, "legs": legs,
                                   "category_id": optional(&client, Named::Category, args.category.as_deref()).await?,
                                   "payee_id": optional(&client, Named::Payee, args.payee.as_deref()).await? });
                let made = client.send(Method::POST, "/api/v1/transactions", Some(&body), None).await?;
                Ok(json!({ "created": made["id"], "date": made["date"], "description": made["description"],
                           "amount": decimal(amount, currency), "currency": currency }))
            }
            .await,
        )
    }

    #[tool(
        description = "Set or remove a transaction's category or payee. What is set here is the person's choice: \
                       rules never change it. WRITES: confirm with the person first.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn update_transaction(
        &self,
        Parameters(args): Parameters<Sorting>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let mut body = Map::new();
                if let Some(category) = args.category.as_deref() {
                    body.insert("category_id".to_owned(), optional(&client, Named::Category, Some(category)).await?);
                }
                if let Some(payee) = args.payee.as_deref() {
                    body.insert("payee_id".to_owned(), optional(&client, Named::Payee, Some(payee)).await?);
                }
                let uri = format!("/api/v1/transactions/{}", args.id.trim());
                let done = client.send(Method::PATCH, &uri, Some(&Value::Object(body)), None).await?;
                Ok(json!({ "updated": done["id"], "category_id": done["category_id"], "payee_id": done["payee_id"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "Set a transaction's category (name or id); rules never change it afterwards. WRITES: \
                       confirm with the person first.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn categorize_transaction(
        &self,
        Parameters(args): Parameters<Categorize>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let category = client.resolve(Named::Category, &args.category).await?;
                let uri = format!("/api/v1/transactions/{}", args.id.trim());
                let body = json!({ "category_id": category["id"] });
                let done = client.send(Method::PATCH, &uri, Some(&body), None).await?;
                Ok(json!({ "updated": done["id"], "category": category["name"],
                           "category_set_by": done["category_set_by"] }))
            }
            .await,
        )
    }

    #[tool(
        description = "Delete a transaction with one leg (a transfer must be split first). An imported one comes \
                       back if its statement is imported again. DESTRUCTIVE: show what will be deleted and wait \
                       for the person's explicit yes.",
        annotations(
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn delete_transaction(
        &self,
        Parameters(args): Parameters<Id>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let uri = format!("/api/v1/transactions/{}", args.id.trim());
                client.send(Method::DELETE, &uri, None, None).await?;
                Ok(json!({ "deleted": args.id.trim() }))
            }
            .await,
        )
    }

    #[tool(
        description = "Create a category. WRITES: confirm with the person first.",
        annotations(
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn create_category(
        &self,
        Parameters(args): Parameters<NewCategory>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        outcome(
            async {
                let client = self.client(&parts)?;
                let body = json!({ "name": args.name, "kind": args.kind.as_deref().unwrap_or("expense") });
                let made = client
                    .send(Method::POST, "/api/v1/categories", Some(&body), None)
                    .await?;
                Ok(json!({ "created": made["id"], "name": made["name"], "kind": made["kind"] }))
            }
            .await,
        )
    }
}
