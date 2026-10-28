//! The MCP write tools, each judged by the state the API keeps afterwards. The listings and access are in `mcp.rs`.

mod common;

use common::Api;
use common::assistant::{Book, Mcp, book, find};
use common::sorting::transactions;
use serde_json::{Value, json};
use sqlx::PgPool;

/// Each transaction's legs as `[account, minor units]`, by id.
async fn legs(api: &Api, book: &Book, id: &Value) -> Value {
    let items = transactions(api, &book.user, "").await;
    let found = items.iter().find(|t| &t["id"] == id).unwrap();
    let legs = found["legs"].as_array().unwrap();
    json!(
        legs.iter()
            .map(|l| json!([l["account_id"], l["amount_minor"]]))
            .collect::<Vec<_>>()
    )
}

/// The transaction now stored under `id`, if any.
async fn stored(api: &Api, book: &Book, id: &Value) -> Option<Value> {
    let items = transactions(api, &book.user, "").await;
    items.into_iter().find(|t| &t["id"] == id)
}

#[sqlx::test]
async fn transactions_are_written_in_the_account_currency(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());

    // Names in any case; the amount read with the account's decimals.
    let lunch = json!({ "date": "2026-04-28", "kind": "spending", "amount": "12.34", "account": "checking",
                        "description": "LUNCH", "category": "dining", "payee": "MARKET" });
    let lunch = mcp.ok(t, "create_transaction", lunch).await;
    let refund = json!({ "date": "2026-04-28", "kind": "income", "amount": "2000", "account": "Yen",
                         "description": "REFUND" });
    let refund = mcp.ok(t, "create_transaction", refund).await;
    let moved = json!({ "date": "2026-04-29", "kind": "transfer", "amount": "5", "account": "Checking",
                        "to_account": book.savings, "description": "TOP UP" });
    let moved = mcp.ok(t, "create_transaction", moved).await;

    assert_eq!(
        legs(&api, &book, &lunch["created"]).await,
        json!([[book.checking, -1234]])
    );
    assert_eq!(
        legs(&api, &book, &refund["created"]).await,
        json!([[book.yen, 2000]])
    );
    let both = json!([[book.checking, -500], [book.savings, 500]]);
    assert_eq!(legs(&api, &book, &moved["created"]).await, both);
    let made = stored(&api, &book, &lunch["created"]).await.unwrap();
    let sorted = (&made["category_id"], &made["payee_id"], &made["category_set_by"]);
    assert_eq!(
        sorted,
        (&json!(book.dining), &json!(book.market), &json!("manual"))
    );
}

#[sqlx::test]
async fn bad_amounts_and_unknown_names_write_nothing(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let count = transactions(&api, &book.user, "").await.len();
    let spend = |amount: &str, account: &str| json!({ "date": "2026-04-28", "kind": "spending", "amount": amount, "account": account, "description": "X" });

    // Never rounded, never signed: the kind says the direction.
    let refusal = mcp
        .refused(t, "create_transaction", spend("12.345", "Checking"))
        .await;
    assert!(refusal.contains("at most 2 decimals"), "{refusal}");
    let refusal = mcp
        .refused(t, "create_transaction", spend("-3", "Checking"))
        .await;
    assert!(refusal.contains("positive"), "{refusal}");
    let refusal = mcp.refused(t, "create_transaction", spend("1.5", "Yen")).await;
    assert!(refusal.contains("at most 0 decimals"), "{refusal}");
    let unknown = mcp.refused(t, "create_transaction", spend("1", "Wallet")).await;
    assert!(
        unknown.contains("no account named") && unknown.contains("Savings"),
        "{unknown}"
    );
    let odd = json!({ "date": "2026-04-28", "kind": "transfer", "amount": "1", "account": "Checking",
                      "description": "X" });
    assert!(
        mcp.refused(t, "create_transaction", odd)
            .await
            .contains("to_account")
    );
    assert_eq!(transactions(&api, &book.user, "").await.len(), count);
}

#[sqlx::test]
async fn sorting_and_deleting_go_through_the_api(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let items = transactions(&api, &book.user, "").await;
    let id = |description: &str| find(&items, description)["id"].clone();
    let (groceries, cafe, transfer) = (
        id("SUPERMERCADO LA COSTA"),
        id("cafe central"),
        id("MOVE TO SAVINGS"),
    );

    let rent = mcp.ok(t, "create_category", json!({ "name": "Rent" })).await;
    assert_eq!(rent["kind"], "expense");
    let change = json!({ "id": groceries, "payee": "", "category": "Rent" });
    mcp.ok(t, "update_transaction", change).await;
    let after = stored(&api, &book, &groceries).await.unwrap();
    assert_eq!(
        (&after["category_id"], &after["payee_id"]),
        (&rent["created"], &Value::Null)
    );

    let sorted = mcp
        .ok(
            t,
            "categorize_transaction",
            json!({ "id": cafe, "category": "Groceries" }),
        )
        .await;
    assert_eq!(sorted["category_set_by"], "manual");
    let after = stored(&api, &book, &cafe).await.unwrap();
    assert_eq!(after["category_id"], json!(book.groceries));

    mcp.ok(t, "delete_transaction", json!({ "id": cafe })).await;
    assert!(stored(&api, &book, &cafe).await.is_none());
    let refusal = mcp
        .refused(t, "delete_transaction", json!({ "id": transfer }))
        .await;
    assert!(refusal.contains("split it first"), "{refusal}");
    assert!(stored(&api, &book, &transfer).await.is_some());
}

#[sqlx::test]
async fn rules_are_added_in_the_account_currency_and_moved(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());

    let big = json!({ "name": "big ramen", "account": "Yen", "amount_min": "3000", "category": "Dining" });
    assert_eq!(mcp.ok(t, "create_rule", big).await["position"], 2);
    let odd = json!({ "name": "odd", "account": "Yen", "amount_min": "1.5" });
    assert!(
        mcp.refused(t, "create_rule", odd)
            .await
            .contains("at most 0 decimals")
    );
    mcp.ok(t, "move_rule", json!({ "rule": "BIG RAMEN", "position": 1 }))
        .await;
    let rules = book.rest(&api, "/api/v1/rules").await;
    let rows = rules["items"].as_array().unwrap();
    let order: Vec<(&Value, &Value)> = rows
        .iter()
        .map(|r| (&r["name"], &r["when"]["amount_min"]))
        .collect();
    assert_eq!(
        order,
        [
            (&json!("big ramen"), &json!(3000)),
            (&json!("ramen"), &json!(1000))
        ]
    );
}

#[sqlx::test]
async fn a_category_set_through_the_tools_survives_the_rules(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let items = transactions(&api, &book.user, "").await;
    let (cafe, north) = (
        find(&items, "cafe central")["id"].clone(),
        find(&items, "CAFE NORTE")["id"].clone(),
    );

    let cafes = json!({ "name": "cafes", "text": "CAFE", "direction": "out", "category": "Dining" });
    mcp.ok(t, "create_rule", cafes).await;
    mcp.ok(
        t,
        "categorize_transaction",
        json!({ "id": cafe, "category": "Groceries" }),
    )
    .await;

    // The rule matches both cafes, but only the one nobody sorted may change.
    let preview = mcp.ok(t, "preview_rules", json!({})).await;
    let changes = preview["changes"].as_array().unwrap();
    let changed: Vec<&Value> = changes.iter().map(|c| &c["transaction"]).collect();
    assert_eq!(changed, [&north], "{preview}");
    assert_eq!(changes[0]["category"], json!([null, "Dining"]));
    let applied = mcp.ok(t, "apply_rules", json!({ "tag": preview["tag"] })).await;
    assert_eq!(applied["applied"], 1);

    let kept = stored(&api, &book, &cafe).await.unwrap();
    let kept = (&kept["category_id"], &kept["category_set_by"]);
    assert_eq!(kept, (&json!(book.groceries), &json!("manual")));
    let set = stored(&api, &book, &north).await.unwrap();
    assert_eq!(
        (&set["category_id"], &set["category_set_by"]),
        (&json!(book.dining), &json!("rule"))
    );
    // The same tag again describes a ledger that has changed since: refused.
    assert!(
        mcp.tool(t, "apply_rules", json!({ "tag": preview["tag"] }))
            .await
            .is_err()
    );
}
