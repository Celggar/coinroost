//! What an assistant needs from the API beyond the web app: balances, text search and deleting a transaction
//! (the server's version in `/health` is checked in `session.rs`).

mod common;

use axum::http::StatusCode;
use common::sorting::{id, post};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn account(api: &Api, user: &User, name: &str) -> String {
    let body =
        json!({ "name": name, "kind": "checking", "currency": "USD", "namespace": name.to_lowercase() });
    id(&post(api, user, "/api/v1/accounts", body).await).to_owned()
}

async fn spend(api: &Api, user: &User, account: &str, date: &str, cents: i64, text: &str) -> String {
    let body = json!({ "date": date, "description": text, "legs": [{ "account_id": account, "amount_minor": cents }] });
    id(&post(api, user, "/api/v1/transactions", body).await).to_owned()
}

fn descriptions(list: &Value) -> Vec<&str> {
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["description"].as_str().unwrap())
        .collect()
}

#[sqlx::test]
async fn an_account_shows_the_sum_of_its_legs(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Balances").await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    spend(&api, &user, &checking, "2026-05-01", 150_000, "SALARY").await;
    spend(&api, &user, &checking, "2026-05-02", -4550, "MARKET").await;
    let transfer = json!({ "date": "2026-05-03", "description": "MOVE", "legs": [
        { "account_id": checking, "amount_minor": -20000 }, { "account_id": savings, "amount_minor": 20000 }] });
    post(&api, &user, "/api/v1/transactions", transfer).await;
    let (_, list) = api.call("GET", "/api/v1/accounts", Some(&user.token), None).await;
    let balances: Vec<(&str, i64)> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| (a["name"].as_str().unwrap(), a["balance_minor"].as_i64().unwrap()))
        .collect();
    // 1,500.00 − 45.50 − 200.00 on checking; the 200.00 on savings.
    assert_eq!(balances, [("Checking", 125_450), ("Savings", 20000)]);
}

#[sqlx::test]
async fn text_search_ignores_case_and_scopes_to_the_household(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Search").await;
    let other = api.household("Neighbour").await;
    let checking = account(&api, &user, "Checking").await;
    spend(&api, &user, &checking, "2026-05-01", -1000, "Super Market SOL").await;
    spend(&api, &user, &checking, "2026-05-02", -2000, "CAFE LUNA").await;
    spend(&api, &user, &checking, "2026-05-03", -3000, "MARKET 100%").await;
    let theirs = account(&api, &other, "Checking").await;
    spend(
        &api,
        &other,
        &theirs,
        "2026-05-01",
        -999,
        "MARKET OF THE NEIGHBOUR",
    )
    .await;

    let (status, found) = api
        .call("GET", "/api/v1/transactions?q=market", Some(&user.token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{found}");
    assert_eq!(descriptions(&found), ["MARKET 100%", "Super Market SOL"]);
    // LIKE wildcards are plain text here.
    let (_, percent) = api
        .call("GET", "/api/v1/transactions?q=100%25", Some(&user.token), None)
        .await;
    assert_eq!(descriptions(&percent), ["MARKET 100%"]);
    let (_, none) = api
        .call("GET", "/api/v1/transactions?q=_", Some(&user.token), None)
        .await;
    assert_eq!(descriptions(&none), Vec::<&str>::new());
    let (status, _) = api
        .call("GET", "/api/v1/transactions?q=%20", Some(&user.token), None)
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn deleting_refuses_transfers_and_other_households(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Deleter").await;
    let other = api.household("Stranger").await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    let coffee = spend(&api, &user, &checking, "2026-05-01", -450, "COFFEE").await;
    let transfer = json!({ "date": "2026-05-03", "description": "MOVE", "legs": [
        { "account_id": checking, "amount_minor": -20000 }, { "account_id": savings, "amount_minor": 20000 }] });
    let moved = id(&post(&api, &user, "/api/v1/transactions", transfer).await).to_owned();

    let uri = |id: &str| format!("/api/v1/transactions/{id}");
    assert_eq!(
        api.call("DELETE", &uri(&coffee), Some(&other.token), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, problem) = api.call("DELETE", &uri(&moved), Some(&user.token), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert!(problem["detail"].as_str().unwrap().contains("split it first"));
    assert_eq!(
        api.call("DELETE", &uri(&coffee), Some(&user.token), None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        api.call("DELETE", &uri(&coffee), Some(&user.token), None).await.0,
        StatusCode::NOT_FOUND
    );

    let (_, left) = api
        .call("GET", "/api/v1/transactions", Some(&user.token), None)
        .await;
    assert_eq!(descriptions(&left), ["MOVE"]);
    let (_, accounts) = api.call("GET", "/api/v1/accounts", Some(&user.token), None).await;
    assert_eq!(
        accounts["items"][0]["balance_minor"], -20000,
        "the coffee's leg went with it"
    );
}
