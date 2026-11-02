//! The transaction list a page at a time: `total` counts every match and `next_cursor` leads to the rest, so a client
//! that reads one page knows the list goes on (an assistant took the 50 rows it got for all 336 uncategorized ones).

mod common;

use axum::http::StatusCode;
use common::assistant::{Mcp, book};
use common::sorting::{id, post};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn get(api: &Api, user: &User, query: &str) -> (StatusCode, Value) {
    api.call(
        "GET",
        &format!("/api/v1/transactions?{query}"),
        Some(&user.token),
        None,
    )
    .await
}

fn item_ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect()
}

#[sqlx::test]
async fn pages_hold_every_match_once_even_when_rows_tie(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Pages").await;
    let account =
        json!({ "name": "Checking", "kind": "checking", "currency": "USD", "namespace": "checking" });
    let checking = id(&post(&api, &user, "/api/v1/accounts", account).await).to_owned();
    let rows = (0..7)
        .map(|n| ("2026-05-02", format!("MARKET {n}")))
        .chain((0..2).map(|n| ("2026-05-01", format!("MARKET OLD {n}"))))
        .chain((0..3).map(|n| ("2026-05-02", format!("SALARY {n}"))));
    for (date, text) in rows {
        let body = json!({ "date": date, "description": text, "legs": [{ "account_id": checking, "amount_minor": -100 }] });
        post(&api, &user, "/api/v1/transactions", body).await;
    }
    // An import records its rows in one database transaction: same date, same instant, only the id tells them apart.
    sqlx::query("update transactions set created_at = '2026-05-03T00:00:00Z'")
        .execute(&db)
        .await
        .unwrap();

    let (_, whole) = get(&api, &user, "q=market&limit=100").await;
    assert_eq!(whole["total"], 9);
    assert!(whole["next_cursor"].is_null(), "one page holds them all: {whole}");
    let mut seen = Vec::new();
    let mut query = "q=market&limit=2".to_owned();
    for page_number in 1.. {
        let (status, page) = get(&api, &user, &query).await;
        assert_eq!(status, StatusCode::OK, "page {page_number}: {page}");
        assert_eq!(
            page["total"], 9,
            "page {page_number} counts every match, not its own rows"
        );
        seen.extend(item_ids(&page));
        let Some(cursor) = page["next_cursor"].as_str() else {
            assert_eq!(page_number, 5, "9 rows in pages of 2");
            break;
        };
        query = format!("q=market&limit=2&cursor={cursor}");
    }
    assert_eq!(
        seen,
        item_ids(&whole),
        "every match once, in the list's order, and no SALARY row"
    );
}

#[sqlx::test]
async fn a_cursor_the_api_did_not_give_is_refused(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Cursors").await;
    // Not base64; base64 of the wrong shape; the right shape with a date that does not exist.
    for cursor in [
        "not.a.cursor",
        "eC55Lno",
        "MjAyNi0wMi0zMC4wLjAwMDAwMDAwLTAwMDAtMDAwMC0wMDAwLTAwMDAwMDAwMDAwMA",
    ] {
        let (status, body) = get(&api, &user, &format!("cursor={cursor}")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{cursor}: {body}");
        assert!(body.to_string().contains("cursor_invalid"), "{cursor}: {body}");
    }
}

#[sqlx::test]
async fn the_assistant_learns_how_many_match_and_fetches_the_rest(db: PgPool) {
    let api = Api::new(db.clone());
    let book = book(&api).await;
    let mcp = Mcp::new(&db);
    let first = mcp
        .ok(
            &book.token,
            "list_transactions",
            json!({ "uncategorized": true, "limit": 1 }),
        )
        .await;
    assert_eq!(first["total"], 2, "{first}");
    assert_eq!(first["transactions"].as_array().unwrap().len(), 1);
    let cursor = first["next_cursor"].as_str().expect("a cursor while more follow");
    let rest = mcp
        .ok(
            &book.token,
            "list_transactions",
            json!({ "uncategorized": true, "limit": 1, "cursor": cursor }),
        )
        .await;
    assert!(rest["next_cursor"].is_null(), "{rest}");
    let both: Vec<&Value> = [&first, &rest]
        .iter()
        .map(|p| &p["transactions"][0]["description"])
        .collect();
    assert_eq!(both, [&json!("CAFE NORTE"), &json!("cafe central")]);

    let found = mcp
        .ok(
            &book.token,
            "search_transactions",
            json!({ "text": "cafe", "limit": 1 }),
        )
        .await;
    assert_eq!(found["total"], 2, "{found}");
    assert!(found["next_cursor"].is_string(), "{found}");
}
