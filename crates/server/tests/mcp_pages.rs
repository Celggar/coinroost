//! The MCP reads that could outgrow one answer: rules a page at a time with their total, transactions by category
//! or payee name with their sums, and no list longer than 50 rows, whatever limit is asked for.

mod common;

use common::Api;
use common::assistant::{Book, Mcp, book, descriptions, ids};
use common::sorting::post;
use serde_json::{Value, json};
use sqlx::PgPool;

/// `count` more rules after the book's one, named `shop 1`, `shop 2`…
async fn shops(api: &Api, book: &Book, count: usize) {
    for n in 1..=count {
        let rule = json!({ "name": format!("shop {n}"), "when": { "description_contains": format!("SHOP {n}") },
                           "then": { "category_id": book.groceries } });
        post(api, &book.user, "/api/v1/rules", rule).await;
    }
}

fn rows(answer: &Value, key: &str) -> usize {
    answer[key].as_array().unwrap().len()
}

#[sqlx::test]
async fn rules_come_25_at_a_time_with_the_total_and_a_cursor(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    shops(&api, &book, 29).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());

    let first = mcp.ok(t, "list_rules", json!({})).await;
    assert_eq!(
        (rows(&first, "rules"), &first["total"]),
        (25, &json!(30)),
        "{first}"
    );
    let cursor = first["next_cursor"].as_str().expect("a cursor while more follow");
    let rest = mcp.ok(t, "list_rules", json!({ "cursor": cursor })).await;
    assert_eq!((rows(&rest, "rules"), &rest["total"]), (5, &json!(30)), "{rest}");
    assert!(rest["next_cursor"].is_null(), "{rest}");
    let mut seen = ids(&first, "rules");
    seen.extend(ids(&rest, "rules"));
    assert_eq!(
        seen,
        ids(&book.rest(&api, "/api/v1/rules").await, "items"),
        "every rule once, in order"
    );

    let found = mcp.ok(t, "list_rules", json!({ "text": "SHOP 2" })).await;
    let names: Vec<&Value> = found["rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| &r["name"])
        .collect();
    // `SHOP 2` is in `SHOP 2` and in `SHOP 20` to `SHOP 29`.
    assert_eq!((names.len(), &found["total"]), (11, &json!(11)), "{found}");
    assert_eq!(names[0], "shop 2");
}

#[sqlx::test]
async fn transactions_by_category_or_payee_name_come_with_their_sums(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let usd = |amount: &str| json!([{ "currency": "USD", "amount": amount }]);

    let groceries = mcp
        .ok(t, "list_transactions", json!({ "category": "groceries" }))
        .await;
    assert_eq!(descriptions(&groceries), ["SUPERMERCADO LA COSTA"]);
    assert_eq!(groceries["sums"], usd("-45.50"));
    let by_id = mcp
        .ok(t, "list_transactions", json!({ "category": book.groceries }))
        .await;
    assert_eq!(descriptions(&by_id), ["SUPERMERCADO LA COSTA"]);
    let market = mcp.ok(t, "list_transactions", json!({ "payee": "Market" })).await;
    assert_eq!(
        (descriptions(&market), &market["sums"]),
        (vec!["SUPERMERCADO LA COSTA"], &usd("-45.50"))
    );
    let dining = mcp
        .ok(t, "list_transactions", json!({ "category": "Dining" }))
        .await;
    let yen = json!([{ "currency": "JPY", "amount": "-1500" }]);
    assert_eq!((descriptions(&dining), &dining["sums"]), (vec!["RAMEN"], &yen));

    // One row on the page; the sums are April's, the move to savings left out.
    let april = json!({ "from": "2026-04-01", "to": "2026-04-30", "limit": 1 });
    let page = mcp.ok(t, "list_transactions", april).await;
    let both = json!([{ "currency": "JPY", "amount": "-1500" }, { "currency": "USD", "amount": "733.50" }]);
    assert_eq!((rows(&page, "transactions"), &page["sums"]), (1, &both));

    let found = mcp
        .ok(
            t,
            "search_transactions",
            json!({ "text": "a", "payee": "market", "category": "Groceries" }),
        )
        .await;
    assert_eq!(
        (descriptions(&found), &found["sums"]),
        (vec!["SUPERMERCADO LA COSTA"], &usd("-45.50"))
    );
    let unknown = mcp
        .refused(t, "list_transactions", json!({ "category": "Travel" }))
        .await;
    assert!(
        unknown.contains("no category named") && unknown.contains("Dining"),
        "{unknown}"
    );
}

#[sqlx::test]
async fn no_list_is_longer_than_50(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    shops(&api, &book, 55).await;
    for n in 1..=55 {
        let row = json!({ "date": "2026-05-01", "description": format!("KIOSK {n}"),
                          "legs": [{ "account_id": book.checking, "amount_minor": -100 }] });
        post(&api, &book.user, "/api/v1/transactions", row).await;
    }
    let (mcp, t) = (Mcp::new(&db), book.token.as_str());
    let many = json!({ "limit": 500 });

    let listed = mcp.ok(t, "list_transactions", many.clone()).await;
    assert_eq!(
        (rows(&listed, "transactions"), &listed["total"]),
        (50, &json!(61))
    );
    let found = mcp
        .ok(t, "search_transactions", json!({ "text": "kiosk", "limit": 500 }))
        .await;
    assert_eq!((rows(&found, "transactions"), &found["total"]), (50, &json!(55)));
    let rules = mcp.ok(t, "list_rules", many).await;
    assert_eq!((rows(&rules, "rules"), &rules["total"]), (50, &json!(56)));
    let default = mcp.ok(t, "list_transactions", json!({})).await;
    assert_eq!(rows(&default, "transactions"), 25);
}
