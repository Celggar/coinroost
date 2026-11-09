//! An opening balance: what an account held when its history starts. It counts in that account's balance and in no
//! income, spending, report, list sum or pairing, as double-entry books keep it in equity.

mod common;

use axum::http::StatusCode;
use common::Api;
use common::assistant::{Book, book};
use common::sorting::{id, post};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Everything that counts money in April and May: the summary, each report and the list's sums.
async fn counted(api: &Api, book: &Book) -> Vec<Value> {
    let period = "from=2026-04-01&to=2026-05-31";
    let mut seen = Vec::new();
    for path in [
        "summary",
        "reports/monthly",
        "reports/categories",
        "reports/payees",
    ] {
        seen.push(book.rest(api, &format!("/api/v1/{path}?{period}")).await);
    }
    seen.push(book.rest(api, &format!("/api/v1/transactions?{period}")).await["sums"].clone());
    // What statements cover, and what payroll owes from it: an opening is not statement data (pass 75).
    seen.push(book.rest(api, "/api/v1/coverage?today=2026-10-01").await);
    seen.push(book.rest(api, "/api/v1/payroll?as_of=2026-10-01").await);
    seen
}

async fn balances(api: &Api, book: &Book) -> Vec<(String, i64)> {
    let accounts = book.rest(api, "/api/v1/accounts").await;
    let items = accounts["items"].as_array().unwrap();
    items
        .iter()
        .map(|a| {
            (
                a["name"].as_str().unwrap().to_owned(),
                a["balance_minor"].as_i64().unwrap(),
            )
        })
        .collect()
}

async fn unpaired(api: &Api, book: &Book) -> Value {
    let (status, preview) = api
        .call("POST", "/api/v1/transfers/preview", Some(&book.user.token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    preview["unmatched"].clone()
}

#[sqlx::test]
async fn an_opening_balance_counts_in_its_account_only(db: PgPool) {
    let api = Api::new(db);
    let book = book(&api).await;
    let (before, held, waiting) = (
        counted(&api, &book).await,
        balances(&api, &book).await,
        unpaired(&api, &book).await,
    );

    let made = post(
        &api,
        &book.user,
        "/api/v1/categories",
        json!({ "name": "Opening", "kind": "opening_balance" }),
    )
    .await;
    assert_eq!(made["kind"], "opening_balance");
    let (_, listed) = api
        .call("GET", "/api/v1/categories", Some(&book.user.token), None)
        .await;
    let kinds: Vec<&Value> = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| &c["kind"])
        .collect();
    assert!(kinds.contains(&&json!("opening_balance")), "{listed}");
    let (status, _) = api
        .call(
            "POST",
            "/api/v1/categories",
            Some(&book.user.token),
            Some(json!({ "name": "Equity", "kind": "equity" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // In the period, one each way: what checking held, and a debt savings started with. And one on checking, where the
    // employer pays, after its last movement, as an account opened before its first statement is imported.
    for (account, minor, date) in [
        (&book.checking, 50000, "2026-04-01"),
        (&book.savings, -7500, "2026-04-01"),
        (&book.checking, 1000, "2026-09-10"),
    ] {
        let body = json!({ "date": date, "description": "Balance inicial", "category_id": id(&made),
                           "legs": [{ "account_id": account, "amount_minor": minor }] });
        post(&api, &book.user, "/api/v1/transactions", body).await;
    }

    assert_eq!(
        counted(&api, &book).await,
        before,
        "no income, spending, report or sum moves"
    );
    assert_eq!(
        unpaired(&api, &book).await,
        waiting,
        "nothing new waits to be paired"
    );
    let moved: Vec<(String, i64)> = held
        .iter()
        .map(|(name, minor)| {
            let opening = match name.as_str() {
                "Checking" => 51000,
                "Savings" => -7500,
                _ => 0,
            };
            (name.clone(), minor + opening)
        })
        .collect();
    assert_eq!(
        balances(&api, &book).await,
        moved,
        "each balance holds its opening"
    );
}
