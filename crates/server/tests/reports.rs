//! Monthly, category and payee reports, worked by hand on synthetic transactions.

mod common;

use axum::http::StatusCode;
use common::sorting::{id, post};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

struct Book {
    groceries: String,
    dining: String,
    market: String,
    cafe: String,
}

async fn make(api: &Api, user: &User, uri: &str, body: Value) -> String {
    id(&post(api, user, uri, body).await).to_owned()
}

/// April and May 2026 in USD and EUR, with a refund, an uncategorized purchase, an unpaired transfer leg, a paired
/// transfer and a June purchase outside the period.
async fn book(api: &Api, user: &User) -> Book {
    let account = |name: &str, currency: &str| json!({ "name": name, "kind": "checking", "currency": currency, "namespace": name.to_lowercase() });
    let checking = make(api, user, "/api/v1/accounts", account("Checking", "USD")).await;
    let savings = make(api, user, "/api/v1/accounts", account("Savings", "USD")).await;
    let euros = make(api, user, "/api/v1/accounts", account("Euros", "EUR")).await;
    let category = |name: &str, kind: &str| json!({ "name": name, "kind": kind });
    let b = Book {
        groceries: make(api, user, "/api/v1/categories", category("Groceries", "expense")).await,
        dining: make(api, user, "/api/v1/categories", category("Dining", "expense")).await,
        market: make(api, user, "/api/v1/payees", json!({ "name": "Market" })).await,
        cafe: make(api, user, "/api/v1/payees", json!({ "name": "Cafe" })).await,
    };
    let salary = make(api, user, "/api/v1/categories", category("Salary", "income")).await;
    let own = make(api, user, "/api/v1/categories", category("Own", "transfer")).await;
    let rows: [(&str, &str, i64, Option<&str>, Option<&str>); 11] = [
        (
            "2026-04-03",
            &checking,
            -4550,
            Some(&b.groceries),
            Some(&b.market),
        ),
        (
            "2026-04-20",
            &checking,
            -2000,
            Some(&b.groceries),
            Some(&b.market),
        ),
        ("2026-04-21", &checking, -1200, Some(&b.dining), Some(&b.cafe)),
        ("2026-04-25", &checking, -300, None, None),
        ("2026-04-30", &checking, 150_000, Some(&salary), None),
        ("2026-05-02", &checking, -1200, Some(&b.dining), Some(&b.cafe)),
        (
            "2026-05-03",
            &checking,
            -5000,
            Some(&b.groceries),
            Some(&b.market),
        ),
        ("2026-05-04", &checking, 1250, Some(&b.groceries), Some(&b.market)),
        ("2026-05-05", &checking, -20000, Some(&own), None),
        ("2026-05-07", &euros, -800, Some(&b.dining), Some(&b.cafe)),
        ("2026-06-01", &checking, -999, Some(&b.groceries), Some(&b.market)),
    ];
    for (date, account, cents, category, payee) in rows {
        let body = json!({ "date": date, "description": "SYNTHETIC", "category_id": category, "payee_id": payee,
                           "legs": [{ "account_id": account, "amount_minor": cents }] });
        post(api, user, "/api/v1/transactions", body).await;
    }
    let transfer = json!({ "date": "2026-05-06", "description": "MOVE", "category_id": own, "legs": [
        { "account_id": checking, "amount_minor": -10000 }, { "account_id": savings, "amount_minor": 10000 }] });
    post(api, user, "/api/v1/transactions", transfer).await;
    b
}

async fn report(api: &Api, user: &User, which: &str, extra: &str) -> (StatusCode, Value) {
    let uri = format!("/api/v1/reports/{which}?from=2026-04-01&to=2026-05-31{extra}");
    api.call("GET", &uri, Some(&user.token), None).await
}

#[sqlx::test]
async fn monthly_report_matches_hand_worked_totals(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Monthly").await;
    book(&api, &user).await;
    let (status, body) = report(&api, &user, "monthly", "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // April USD: 45.50 + 20.00 + 12.00 + 3.00 spent, 1500.00 earned. May EUR: 8.00 spent. May USD: 12.00 + 50.00
    // spent and the 12.50 refund earned; the transfers and June count nowhere.
    assert_eq!(
        body["months"],
        json!([
            { "month": "2026-04", "currency": "USD", "spending": 8050, "income": 150_000 },
            { "month": "2026-05", "currency": "EUR", "spending": 800, "income": 0 },
            { "month": "2026-05", "currency": "USD", "spending": 6200, "income": 1250 }
        ])
    );
    let (status, _) = api
        .call(
            "GET",
            "/api/v1/reports/monthly?from=2026-06-01&to=2026-05-01",
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn category_report_matches_hand_worked_totals(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Categories").await;
    let b = book(&api, &user).await;
    let (status, body) = report(&api, &user, "categories", "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Groceries 45.50 + 20.00 + 50.00 (the refund is money in, not less spending); Dining 12.00 twice; the
    // uncategorized 3.00 last; EUR on its own.
    assert_eq!(
        body["items"],
        json!([
            { "category_id": b.dining, "currency": "EUR", "spending": 800, "count": 1 },
            { "category_id": b.groceries, "currency": "USD", "spending": 11550, "count": 3 },
            { "category_id": b.dining, "currency": "USD", "spending": 2400, "count": 2 },
            { "category_id": null, "currency": "USD", "spending": 300, "count": 1 }
        ])
    );
}

#[sqlx::test]
async fn payee_report_matches_hand_worked_totals(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Payees").await;
    let b = book(&api, &user).await;
    let (status, body) = report(&api, &user, "payees", "").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["items"],
        json!([
            { "payee_id": b.cafe, "currency": "EUR", "spending": 800, "count": 1 },
            { "payee_id": b.market, "currency": "USD", "spending": 11550, "count": 3 },
            { "payee_id": b.cafe, "currency": "USD", "spending": 2400, "count": 2 },
            { "payee_id": null, "currency": "USD", "spending": 300, "count": 1 }
        ])
    );
    let (_, top) = report(&api, &user, "payees", "&limit=1").await;
    let firsts: Vec<_> = top["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["payee_id"].clone())
        .collect();
    assert_eq!(
        firsts,
        [json!(b.cafe), json!(b.market)],
        "the limit is per currency"
    );
    assert_eq!(
        report(&api, &user, "payees", "&limit=0").await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
}
