//! An MCP client for the tests (JSON-RPC over HTTP to `/mcp`) and a household seeded with April 2026's money.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use coinroost_server::AppState;
use coinroost_server::employers::{self, EmployersFile};
use coinroost_server::tokens::{self, Scope};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

use super::sorting::{id, post};
use super::{Api, User};

pub struct Mcp {
    app: Router,
}

impl Mcp {
    /// `/mcp` over a fresh database, accepting `Host: localhost`.
    pub fn new(db: &PgPool) -> Mcp {
        Mcp::with_hosts(db, &["localhost".to_owned()])
    }

    pub fn with_hosts(db: &PgPool, hosts: &[String]) -> Mcp {
        Mcp {
            app: coinroost_server::mcp::router(
                AppState {
                    db: db.clone(),
                    first_account: false,
                },
                hosts,
            ),
        }
    }

    /// Posts one JSON-RPC message; returns the HTTP status and the answer, read from a JSON body or from the first
    /// event of a stream.
    pub async fn post(&self, host: &str, headers: &[(&str, &str)], message: &Value) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method("POST")
            .uri("/mcp")
            .header(header::HOST, host)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream");
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let body = Body::from(message.to_string());
        let response = self
            .app
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        let event = text
            .lines()
            .filter_map(|l| l.strip_prefix("data:"))
            .map(str::trim)
            .find(|l| !l.is_empty());
        let answer = match event {
            Some(data) => serde_json::from_str(data).unwrap(),
            None => serde_json::from_str(&text).unwrap_or_else(|_| json!(text)),
        };
        (status, answer)
    }

    /// A JSON-RPC request with a bearer; returns its `result`.
    pub async fn rpc(&self, token: &str, method: &str, params: Value) -> Value {
        let bearer = format!("Bearer {token}");
        let message = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let (status, answer) = self
            .post("localhost", &[("authorization", &bearer)], &message)
            .await;
        assert_eq!(status, StatusCode::OK, "{method}: {answer}");
        assert!(answer["error"].is_null(), "{method}: {answer}");
        answer["result"].clone()
    }

    /// Calls a tool: its JSON answer, or the reason it gave as an error result.
    pub async fn tool(&self, token: &str, name: &str, arguments: Value) -> Result<Value, String> {
        let params = json!({ "name": name, "arguments": arguments });
        let result = self.rpc(token, "tools/call", params).await;
        let text = result["content"][0]["text"].as_str().unwrap().to_owned();
        if result["isError"] == true {
            Err(text)
        } else {
            Ok(serde_json::from_str(&text).unwrap())
        }
    }

    pub async fn ok(&self, token: &str, name: &str, arguments: Value) -> Value {
        let answer = self.tool(token, name, arguments).await;
        answer.unwrap_or_else(|reason| panic!("{name}: {reason}"))
    }

    pub async fn refused(&self, token: &str, name: &str, arguments: Value) -> String {
        match self.tool(token, name, arguments).await {
            Err(reason) => reason,
            Ok(answer) => panic!("{name} should have been refused: {answer}"),
        }
    }
}

/// A household with three accounts, April 2026's money, an employer, a rule, and a write token.
pub struct Book {
    pub user: User,
    pub token: String,
    pub checking: String,
    pub savings: String,
    pub yen: String,
    pub groceries: String,
    pub dining: String,
    pub salary: String,
    pub own: String,
    pub market: String,
}

async fn make(api: &Api, user: &User, uri: &str, body: Value) -> String {
    id(&post(api, user, uri, body).await).to_owned()
}

pub async fn book(api: &Api) -> Book {
    let user = api.household("Assistant").await;
    let account = |name: &str, kind: &str, currency: &str| json!({ "name": name, "kind": kind, "currency": currency, "namespace": name.to_lowercase() });
    let category = |name: &str, kind: &str| json!({ "name": name, "kind": kind });
    let (accounts, categories) = ("/api/v1/accounts", "/api/v1/categories");
    let mut book = Book {
        token: String::new(),
        checking: make(api, &user, accounts, account("Checking", "checking", "USD")).await,
        savings: make(api, &user, accounts, account("Savings", "savings", "USD")).await,
        yen: make(api, &user, accounts, account("Yen", "cash", "JPY")).await,
        groceries: make(api, &user, categories, category("Groceries", "expense")).await,
        dining: make(api, &user, categories, category("Dining", "expense")).await,
        salary: make(api, &user, categories, category("Salary", "income")).await,
        own: make(api, &user, categories, category("Own", "transfer")).await,
        market: make(api, &user, "/api/v1/payees", json!({ "name": "Market" })).await,
        user,
    };
    money(api, &book).await;
    employer_and_rule(api, &book).await;
    book.token = tokens::add(&api.db, &book.user.email, "assistant", Scope::Write)
        .await
        .unwrap();
    book
}

async fn money(api: &Api, b: &Book) {
    let (checking, yen) = (&b.checking, &b.yen);
    let rows = [
        (
            "2026-04-03",
            checking,
            -4550,
            "SUPERMERCADO LA COSTA",
            Some(&b.groceries),
            Some(&b.market),
        ),
        (
            "2026-04-15",
            checking,
            80000,
            "ACME PLANILLA",
            Some(&b.salary),
            None,
        ),
        ("2026-04-20", checking, -1200, "cafe central", None, None),
        ("2026-04-22", checking, -900, "CAFE NORTE", None, None),
        ("2026-04-26", yen, -1500, "RAMEN", Some(&b.dining), None),
    ];
    for (date, account, minor, description, category, payee) in rows {
        let body = json!({ "date": date, "description": description, "category_id": category, "payee_id": payee,
                           "legs": [{ "account_id": account, "amount_minor": minor }] });
        post(api, &b.user, "/api/v1/transactions", body).await;
    }
    let transfer = json!({ "date": "2026-04-25", "description": "MOVE TO SAVINGS", "category_id": b.own, "legs": [
        { "account_id": checking, "amount_minor": -10000 }, { "account_id": b.savings, "amount_minor": 10000 }] });
    post(api, &b.user, "/api/v1/transactions", transfer).await;
}

/// Acme pays into checking from April; a rule on the yen account, whose 1000 is a thousand yen, not ten.
async fn employer_and_rule(api: &Api, b: &Book) {
    let rule = json!({ "name": "ramen", "when": { "account_id": b.yen, "amount_min": 1000, "direction": "out" },
                       "then": { "category_id": b.dining } });
    post(api, &b.user, "/api/v1/rules", rule).await;
    let (_, me) = api.call("GET", "/api/v1/me", Some(&b.user.token), None).await;
    let employer = r#"{ "employers": [ { "name": "Acme", "match": "ACME", "account": "checking",
        "min_amount_minor": 70000, "amount_minor": 80000, "start": "2026-04" } ] }"#;
    let household = me["household_id"].as_str().unwrap().parse().unwrap();
    let file = EmployersFile::from_json(employer).unwrap();
    employers::load(&api.db, household, &file).await.unwrap();
}

impl Book {
    /// What the API answers to a GET with the book's token.
    pub async fn rest(&self, api: &Api, uri: &str) -> Value {
        let (status, body) = api.call("GET", uri, Some(&self.token), None).await;
        assert_eq!(status, StatusCode::OK, "{uri}: {body}");
        body
    }
}

pub fn ids(list: &Value, key: &str) -> Vec<Value> {
    list[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].clone())
        .collect()
}

pub fn descriptions(list: &Value) -> Vec<&str> {
    let rows = list["transactions"].as_array().unwrap();
    rows.iter().map(|t| t["description"].as_str().unwrap()).collect()
}

pub fn find<'a>(items: &'a [Value], description: &str) -> &'a Value {
    items.iter().find(|t| t["description"] == description).unwrap()
}
