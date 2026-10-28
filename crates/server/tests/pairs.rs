//! Pairing by hand, splitting, and the summary that leaves transfers out.

mod common;

use axum::http::StatusCode;
use common::sorting::{id, post};
use common::transfers::*;
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

/// Who acts is one argument: the API and the user.
async fn one_leg(
    (api, user): (&Api, &User),
    date: &str,
    account: &str,
    cents: i64,
    category: Option<&str>,
) -> String {
    let body = json!({ "date": date, "description": format!("MOVE {cents}"), "category_id": category,
                       "legs": [{ "account_id": account, "amount_minor": cents, "statement_text": "BANK TEXT" }] });
    id(&post(api, user, "/api/v1/transactions", body).await).to_owned()
}

async fn pair(api: &Api, user: &User, a: &str, b: &str) -> (StatusCode, Value) {
    api.call(
        "POST",
        "/api/v1/transfers",
        Some(&user.token),
        Some(json!({ "transaction_ids": [a, b] })),
    )
    .await
}

async fn manual_account(api: &Api, user: &User, name: &str, currency: &str) -> String {
    let body =
        json!({ "name": name, "kind": "checking", "currency": currency, "namespace": name.to_lowercase() });
    id(&post(api, user, "/api/v1/accounts", body).await).to_owned()
}

async fn category(api: &Api, user: &User, name: &str, kind: &str) -> String {
    id(&post(
        api,
        user,
        "/api/v1/categories",
        json!({ "name": name, "kind": kind }),
    )
    .await)
    .to_owned()
}

#[sqlx::test]
async fn split_then_pair_restores_the_same_legs(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Undo").await;
    let own = transfer_rule(&api, &user, "Own transfers", "TRANSFER").await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    import(
        &api,
        &user,
        &checking,
        statement(&[("15/05/2026", "TRANSFER TO SAVINGS", -50000)]),
    )
    .await;
    import(
        &api,
        &user,
        &savings,
        statement(&[("18/05/2026", "TRANSFER FROM CHECKING", 50000)]),
    )
    .await;
    let made = transfers(&api, &user).await;
    assert_eq!(made.len(), 1, "Friday to Monday");
    let transfer = made[0]["id"].as_str().unwrap().to_owned();
    let (paired_legs, paired_totals) = (legs(&api, &user).await, balances(&api, &user).await);

    let (status, split) = api
        .call(
            "POST",
            &format!("/api/v1/transactions/{transfer}/split"),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{split}");
    assert_eq!(transfers(&api, &user).await.len(), 0);
    assert_eq!(legs(&api, &user).await, paired_legs, "the same legs, every field");
    assert_eq!(balances(&api, &user).await, paired_totals);
    let inflow = split["transaction_ids"][1].as_str().unwrap();
    let (_, back) = api
        .call("GET", "/api/v1/transactions?limit=10", Some(&user.token), None)
        .await;
    let rebuilt = back["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == inflow)
        .unwrap();
    assert_eq!(
        (&rebuilt["date"], &rebuilt["description"], &rebuilt["category_id"]),
        (
            &json!("2026-05-18"),
            &json!("TRANSFER FROM CHECKING"),
            &json!(own)
        ),
        "as the bank printed it, with the transfer's category"
    );

    let (status, again) = pair(&api, &user, inflow, &transfer).await;
    assert_eq!(status, StatusCode::CREATED, "{again}");
    assert_eq!(
        again["transaction_id"],
        transfer.as_str(),
        "the outflow's transaction"
    );
    assert_eq!(legs(&api, &user).await, paired_legs);
    assert_eq!(balances(&api, &user).await, paired_totals);

    let (status, _) = api
        .call(
            "POST",
            &format!("/api/v1/transactions/{inflow}/split"),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "the inflow's transaction is gone again"
    );
}

#[sqlx::test]
async fn manual_pair_checks_its_legs(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Checked").await;
    let other = api.household("Elsewhere").await;
    let checking = manual_account(&api, &user, "Checking", "USD").await;
    let savings = manual_account(&api, &user, "Savings", "USD").await;
    let euros = manual_account(&api, &user, "Euros", "EUR").await;
    let day = "2026-05-04";
    let out = one_leg((&api, &user), day, &checking, -1000, None).await;
    let same_account = one_leg((&api, &user), day, &checking, 1000, None).await;
    let less = one_leg((&api, &user), day, &savings, 900, None).await;
    let also_out = one_leg((&api, &user), day, &savings, -1000, None).await;
    let in_euros = one_leg((&api, &user), day, &euros, 1000, None).await;
    let theirs = {
        let account = manual_account(&api, &other, "Theirs", "USD").await;
        one_leg((&api, &other), day, &account, 1000, None).await
    };
    let body = json!({ "date": day, "description": "BOTH", "legs": [
        { "account_id": checking, "amount_minor": -1000 }, { "account_id": savings, "amount_minor": 1000 }] });
    let two_legs = id(&post(&api, &user, "/api/v1/transactions", body).await).to_owned();

    for (with, why) in [
        (&out, "different transactions"),
        (&same_account, "two different accounts"),
        (&less, "sum to zero"),
        (&also_out, "sum to zero"),
        (&in_euros, "one currency"),
        (&theirs, "one leg each"),
        (&two_legs, "one leg each"),
    ] {
        let (status, problem) = pair(&api, &user, &out, with).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{why}: {problem}");
        assert!(
            problem["detail"].as_str().unwrap().contains(why),
            "{why}: {problem}"
        );
    }
    let right = one_leg((&api, &user), "2026-05-20", &savings, 1000, None).await;
    assert_eq!(
        pair(&api, &user, &right, &out).await.0,
        StatusCode::CREATED,
        "any date, by hand"
    );
}

#[sqlx::test]
async fn summary_matches_hand_worked_totals(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Totals").await;
    let checking = manual_account(&api, &user, "Checking", "USD").await;
    let savings = manual_account(&api, &user, "Savings", "USD").await;
    let euros = manual_account(&api, &user, "Euros", "EUR").await;
    let groceries = category(&api, &user, "Groceries", "expense").await;
    let salary = category(&api, &user, "Salary", "income").await;
    let own = category(&api, &user, "Own transfers", "transfer").await;
    let may = "2026-05-10";
    for (date, account, cents, category) in [
        (may, &checking, -4550, Some(&groceries)),
        (may, &checking, -4550, Some(&groceries)),
        (may, &checking, -375, None),
        (may, &checking, 146_216, Some(&salary)),
        (may, &checking, 1250, Some(&groceries)),
        (may, &checking, -15000, Some(&own)),
        (may, &savings, 2000, Some(&own)),
        ("2026-06-01", &checking, -999, Some(&groceries)),
        (may, &euros, -1000, Some(&groceries)),
    ] {
        one_leg((&api, &user), date, account, cents, category.map(String::as_str)).await;
    }
    let out = one_leg((&api, &user), may, &checking, -50000, Some(&own)).await;
    let inn = one_leg((&api, &user), may, &savings, 50000, Some(&own)).await;
    assert_eq!(pair(&api, &user, &out, &inn).await.0, StatusCode::CREATED);

    let (status, summary) = api
        .call(
            "GET",
            "/api/v1/summary?from=2026-05-01&to=2026-05-31",
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{summary}");
    // Worked by hand. USD spending: 45.50 + 45.50 + 3.75 (uncategorized) = 94.75; income: 1462.16 + 12.50
    // (a refund comes in) = 1474.66; unpaired transfer legs 150.00 out and 20.00 in; the paired 500.00 and
    // June's 9.99 count nowhere. EUR: 10.00 spent.
    assert_eq!(
        summary["totals"],
        json!([
            { "currency": "EUR", "spending": 1000, "income": 0, "unpaired_transfer_out": 0,
              "unpaired_transfer_in": 0 },
            { "currency": "USD", "spending": 9475, "income": 147_466, "unpaired_transfer_out": 15000,
              "unpaired_transfer_in": 2000 }
        ])
    );
    let (status, _) = api
        .call(
            "GET",
            "/api/v1/summary?from=2026-06-01&to=2026-05-01",
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}
