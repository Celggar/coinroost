//! The MCP endpoint spoken over HTTP, as an MCP client would: what it lists, the read tools against the API's own
//! answers and numbers worked by hand, and who is let in. The write tools are in `mcp_writes.rs`.

mod common;

use axum::http::StatusCode;
use coinroost_server::tokens::{self, Scope};
use common::Api;
use common::assistant::{Mcp, book, descriptions, find, ids};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Every tool of the M7 table, sorted.
const TOOLS: [&str; 19] = [
    "about",
    "apply_rules",
    "categorize_transaction",
    "create_category",
    "create_rule",
    "create_transaction",
    "delete_transaction",
    "list_accounts",
    "list_categories",
    "list_rules",
    "list_transactions",
    "monthly_trend",
    "move_rule",
    "payroll_status",
    "preview_rules",
    "search_transactions",
    "spending_summary",
    "top_payees",
    "update_transaction",
];

async fn tools(db: &PgPool, token: &str) -> Vec<Value> {
    let listed = Mcp::new(db).rpc(token, "tools/list", json!({})).await;
    listed["tools"].as_array().unwrap().clone()
}

#[sqlx::test]
async fn it_introduces_itself_and_lists_every_tool(db: PgPool) {
    let book = book(&Api::new(db.clone())).await;
    let hello = json!({ "protocolVersion": "2025-06-18", "capabilities": {},
                        "clientInfo": { "name": "test", "version": "1" } });
    let hello = Mcp::new(&db).rpc(&book.token, "initialize", hello).await;
    assert_eq!(hello["serverInfo"]["name"], "coinroost");
    assert_eq!(hello["serverInfo"]["version"], env!("CARGO_PKG_VERSION"));
    assert!(hello["instructions"].as_str().unwrap().contains("explicit yes"));

    let tools = tools(&db, &book.token).await;
    let mut names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    assert_eq!(names, TOOLS);
    let create = tools.iter().find(|t| t["name"] == "create_transaction").unwrap();
    let required = create["inputSchema"]["required"].as_array().unwrap();
    let mut required: Vec<&str> = required.iter().map(|r| r.as_str().unwrap()).collect();
    required.sort_unstable();
    assert_eq!(required, ["account", "amount", "date", "description", "kind"]);
}

#[sqlx::test]
async fn every_writer_says_so_and_only_delete_destroys(db: PgPool) {
    let book = book(&Api::new(db.clone())).await;
    for tool in tools(&db, &book.token).await {
        let (name, hints) = (&tool["name"], &tool["annotations"]);
        assert_eq!(hints["openWorldHint"], false, "{name}");
        assert!(hints["readOnlyHint"].is_boolean(), "{name}");
        if hints["readOnlyHint"] == true {
            continue;
        }
        let description = tool["description"].as_str().unwrap();
        let warns = description.contains("WRITES") || description.contains("DESTRUCTIVE");
        assert!(warns, "{name}: {description}");
        assert!(hints["idempotentHint"].is_boolean(), "{name}");
        assert_eq!(
            hints["destructiveHint"] == true,
            name == "delete_transaction",
            "{name}"
        );
    }
}

#[sqlx::test]
async fn accounts_categories_and_rules_match_the_api(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());

    // Worked by hand: checking -45.50 + 800.00 - 12.00 - 9.00 - 100.00; the yen account in whole yen.
    let accounts = mcp.ok(t, "list_accounts", json!({})).await;
    let from_api = book.rest(&api, "/api/v1/accounts").await;
    assert_eq!(ids(&accounts, "accounts"), ids(&from_api, "items"));
    let rows = accounts["accounts"].as_array().unwrap();
    let balances: Vec<(&Value, &Value)> = rows.iter().map(|a| (&a["name"], &a["balance"])).collect();
    for (name, balance) in [("Checking", "633.50"), ("Savings", "100.00"), ("Yen", "-1500")] {
        assert!(
            balances.contains(&(&json!(name), &json!(balance))),
            "{balances:?}"
        );
    }
    let savings = mcp.ok(t, "list_accounts", json!({ "kind": "savings" })).await;
    assert_eq!(ids(&savings, "accounts"), [json!(book.savings)]);

    let categories = mcp.ok(t, "list_categories", json!({})).await;
    let from_api = book.rest(&api, "/api/v1/categories").await;
    let expected: Vec<Value> = from_api["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| json!({ "id": c["id"], "name": c["name"], "kind": c["kind"] }))
        .collect();
    assert_eq!(categories["categories"], json!(expected));

    let rules = mcp.ok(t, "list_rules", json!({})).await;
    assert_eq!(
        ids(&rules, "rules"),
        ids(&book.rest(&api, "/api/v1/rules").await, "items")
    );
    let when = json!({ "text": null, "direction": "out", "account": "Yen", "amount_min": "1000", "amount_max": null });
    assert_eq!(rules["rules"][0]["when"], when, "a yen rule's amount in yen");
    assert_eq!(rules["rules"][0]["then"]["category"], "Dining");
}

#[sqlx::test]
async fn transactions_and_about_match_the_api(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let same = async |tool: &Value, query: &str| {
        let from_api = book.rest(&api, &format!("/api/v1/transactions{query}")).await;
        assert_eq!(ids(tool, "transactions"), ids(&from_api, "items"), "{query}");
    };

    let all = mcp.ok(t, "list_transactions", json!({})).await;
    same(&all, "?limit=50").await;
    let groceries = find(all["transactions"].as_array().unwrap(), "SUPERMERCADO LA COSTA");
    let sorted = (
        &groceries["category"],
        &groceries["category_set_by"],
        &groceries["payee"],
    );
    assert_eq!(sorted, (&json!("Groceries"), &json!("manual"), &json!("Market")));
    let legs = json!([{ "account": "Checking", "amount": "-45.50", "currency": "USD" }]);
    assert_eq!(
        (&groceries["date"], &groceries["legs"]),
        (&json!("2026-04-03"), &legs)
    );

    let unsorted = mcp
        .ok(t, "list_transactions", json!({ "uncategorized": true }))
        .await;
    assert_eq!(descriptions(&unsorted), ["CAFE NORTE", "cafe central"]);
    same(&unsorted, "?uncategorized=true").await;
    let by_name = mcp
        .ok(t, "list_transactions", json!({ "account": "savings" }))
        .await;
    assert_eq!(descriptions(&by_name), ["MOVE TO SAVINGS"]);
    let found = mcp.ok(t, "search_transactions", json!({ "text": "Cafe" })).await;
    assert_eq!(descriptions(&found), ["CAFE NORTE", "cafe central"]);
    same(&found, "?q=Cafe").await;

    let about = mcp.ok(t, "about", json!({})).await;
    let me = book.rest(&api, "/api/v1/me").await;
    let expected = json!({ "version": env!("CARGO_PKG_VERSION"), "email": book.user.email,
                           "household_id": me["household_id"] });
    assert_eq!(about, expected);
}

#[sqlx::test]
async fn the_summary_and_the_trend_match_hand_worked_numbers(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let april = json!({ "from": "2026-04-01", "to": "2026-04-30" });

    // USD: 45.50 + 12.00 + 9.00 spent, 800.00 earned; the move to savings is neither. JPY: 1500 spent.
    let summary = mcp.ok(t, "spending_summary", april.clone()).await;
    let currencies = summary["currencies"].as_array().unwrap();
    let currency = |c: &str| currencies.iter().find(|x| x["currency"] == c).unwrap().clone();
    let usd = json!({ "currency": "USD", "spending": "66.50", "income": "800.00", "income_minus_spending": "733.50",
                      "unpaired_transfer_out": "0.00", "unpaired_transfer_in": "0.00", "spending_by_category": [
                          { "category": "Groceries", "spending": "45.50", "count": 1 },
                          { "category": "(no category)", "spending": "21.00", "count": 2 }] });
    assert_eq!(currency("USD"), usd);
    let dining = json!([{ "category": "Dining", "spending": "1500", "count": 1 }]);
    assert_eq!(currency("JPY")["spending_by_category"], dining);

    let trend = mcp.ok(t, "monthly_trend", april).await;
    let months = json!([{ "month": "2026-04", "currency": "JPY", "spending": "1500", "income": "0" },
                        { "month": "2026-04", "currency": "USD", "spending": "66.50", "income": "800.00" }]);
    assert_eq!(trend["months"], months);
    let coverage = book.rest(&api, "/api/v1/coverage?today=2026-04-30").await;
    let column = |list: &Value, key: &str, field: &str| -> Vec<Value> {
        list[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a[field].clone())
            .collect()
    };
    assert_eq!(
        column(&trend, "coverage", "account"),
        column(&coverage, "items", "name")
    );
}

#[sqlx::test]
async fn payees_and_payroll_match_the_api(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());

    let payees = mcp
        .ok(
            t,
            "top_payees",
            json!({ "from": "2026-04-01", "to": "2026-04-30" }),
        )
        .await;
    let expected = json!([{ "payee": "(no payee)", "currency": "JPY", "spending": "1500", "count": 1 },
                          { "payee": "Market", "currency": "USD", "spending": "45.50", "count": 1 },
                          { "payee": "(no payee)", "currency": "USD", "spending": "21.00", "count": 2 }]);
    assert_eq!(payees["payees"], expected);

    let payroll = mcp
        .ok(t, "payroll_status", json!({ "as_of": "2026-05-10" }))
        .await;
    let from_api = book.rest(&api, "/api/v1/payroll?as_of=2026-05-10").await;
    let (acme, api_acme) = (&payroll["employers"][0], &from_api["employers"][0]);
    // April's first half was paid on the 15th; nothing is owed.
    let counted = (&acme["employer"], &acme["payments_counted"], &acme["owed_total"]);
    assert_eq!(counted, (&json!("Acme"), &json!(1), &json!("0.00")));
    assert_eq!(
        (&acme["data_until"], &acme["payments_counted"]),
        (&api_acme["data_until"], &api_acme["paid"])
    );
    let count = |v: &Value| v.as_array().unwrap().len();
    assert_eq!(
        (count(&acme["owed"]), count(&acme["no_data_yet"])),
        (count(&api_acme["owed"]), count(&api_acme["unknown"]))
    );
}

#[sqlx::test]
async fn only_a_bearer_from_outside_a_browser_gets_in(db: PgPool) {
    let book = book(&Api::new(db.clone())).await;
    let mcp = Mcp::new(&db);
    let list = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
    let status = async |host: &str, headers: &[(&str, &str)]| mcp.post(host, headers, &list).await.0;
    let bearer = format!("Bearer {}", book.token);

    assert_eq!(
        status("localhost", &[("authorization", &bearer)]).await,
        StatusCode::OK
    );
    assert_eq!(status("localhost", &[]).await, StatusCode::UNAUTHORIZED);
    let cookie = format!("coinroost_session={}", book.user.token);
    let from_cookie = status("localhost", &[("cookie", &cookie)]).await;
    assert_eq!(
        from_cookie,
        StatusCode::UNAUTHORIZED,
        "the browser session never opens /mcp"
    );
    let forged = format!("Bearer crt_{}", "A".repeat(43));
    assert_eq!(
        status("localhost", &[("authorization", &forged)]).await,
        StatusCode::UNAUTHORIZED
    );
    let page = [("authorization", bearer.as_str()), ("origin", "http://localhost")];
    assert_eq!(status("localhost", &page).await, StatusCode::FORBIDDEN);
    let elsewhere = status("evil.example", &[("authorization", &bearer)]).await;
    assert!(elsewhere.is_client_error(), "{elsewhere}");

    assert!(tokens::revoke(&db, &book.user.email, "assistant").await.unwrap());
    assert_eq!(
        status("localhost", &[("authorization", &bearer)]).await,
        StatusCode::UNAUTHORIZED
    );
}

#[sqlx::test]
async fn no_configured_host_means_loopback_not_any_host(db: PgPool) {
    let book = book(&Api::new(db.clone())).await;
    let list = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {} });
    let bearer = format!("Bearer {}", book.token);
    let open = async |hosts: &[&str], host: &str| {
        let hosts: Vec<String> = hosts.iter().map(|h| (*h).to_owned()).collect();
        let mcp = Mcp::with_hosts(&db, &hosts);
        mcp.post(host, &[("authorization", &bearer)], &list).await.0
    };
    // Nothing configured: loopback on any port, never another name.
    assert_eq!(open(&[], "127.0.0.1:8080").await, StatusCode::OK);
    assert_eq!(open(&[], "localhost:8080").await, StatusCode::OK);
    assert!(open(&[], "evil.example:8080").await.is_client_error());
    // A name alone accepts any port; with a port, only that one.
    assert_eq!(
        open(&["money.example"], "money.example:443").await,
        StatusCode::OK
    );
    assert!(
        open(&["money.example:8443"], "money.example:443")
            .await
            .is_client_error()
    );
    assert!(open(&["money.example"], "127.0.0.1:8080").await.is_client_error());
}

#[sqlx::test]
async fn a_read_token_reads_and_the_api_refuses_its_writes(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let mcp = Mcp::new(&db);
    let reader = tokens::add(&db, &book.user.email, "reader", Scope::Read)
        .await
        .unwrap();

    mcp.ok(&reader, "list_accounts", json!({})).await;
    let categories = async || {
        book.rest(&api, "/api/v1/categories").await["items"]
            .as_array()
            .unwrap()
            .len()
    };
    let before = categories().await;
    let refusal = mcp
        .refused(&reader, "create_category", json!({ "name": "Sneaky" }))
        .await;
    assert!(
        refusal.contains("403") && refusal.contains("only read"),
        "{refusal}"
    );
    assert_eq!(categories().await, before);
}
