//! Helpers for the transfer tests: accounts read with the BAC account profile, synthetic statements in that
//! format, a transfer category sorted by a rule, and the transfer preview.

use std::collections::BTreeMap;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use serde_json::{Value, json};

use super::sorting::{id, post, transactions};
use super::{Api, User};

/// An account whose statements are BAC account CSV files.
pub async fn account(api: &Api, user: &User, name: &str) -> String {
    let body = json!({ "name": name, "kind": "checking", "currency": "USD",
                       "namespace": name.to_lowercase(), "profile": "bac-account" });
    id(&post(api, user, "/api/v1/accounts", body).await).to_owned()
}

/// A BAC account statement with these rows: (day/month/year, description, signed amount in cents).
pub fn statement(rows: &[(&str, &str, i64)]) -> Vec<u8> {
    // The fixture's summary and table header, kept as bytes: the bank writes Windows-1252.
    let fixture: &[u8] = include_bytes!("../../../../spec/fixtures/bac-account/input.csv");
    let mut file: Vec<u8> = Vec::new();
    for line in fixture.split(|&b| b == b'\n').take(5) {
        file.extend_from_slice(line);
        file.push(b'\n');
    }
    let money = |c: i64| format!("{}.{:02}", c / 100, c % 100);
    for (number, (date, description, cents)) in rows.iter().enumerate() {
        let (debit, credit) = if *cents < 0 {
            (money(-cents), money(0))
        } else {
            (money(0), money(*cents))
        };
        let row = format!(
            "{date}, {}, TR, {description}, {debit}, {credit}, 0.00 \r\n",
            200_001 + number
        );
        file.extend_from_slice(row.as_bytes());
    }
    file.extend_from_slice(b"\r\n");
    file
}

/// Imports a statement into an account; returns the import report.
pub async fn import(api: &Api, user: &User, account_id: &str, file: Vec<u8>) -> Value {
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/accounts/{account_id}/imports?file_name=statement.csv"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", user.token))
        .body(Body::from(file))
        .unwrap();
    let (status, report) = api.send(request).await;
    assert_eq!(status, StatusCode::CREATED, "{report}");
    report
}

/// A category of kind transfer, and a rule that gives it to every description containing `text`.
pub async fn transfer_rule(api: &Api, user: &User, name: &str, text: &str) -> String {
    let category = id(&post(
        api,
        user,
        "/api/v1/categories",
        json!({ "name": name, "kind": "transfer" }),
    )
    .await)
    .to_owned();
    post(
        api,
        user,
        "/api/v1/rules",
        json!({ "name": name, "when": { "description_contains": text }, "then": { "category_id": category } }),
    )
    .await;
    category
}

/// The transfers: transactions with two legs.
pub async fn transfers(api: &Api, user: &User) -> Vec<Value> {
    transactions(api, user, "&transfers=true").await
}

/// Every leg of the household, by id, with all its fields: what pairing and splitting must never change.
pub async fn legs(api: &Api, user: &User) -> BTreeMap<String, Value> {
    transactions(api, user, "")
        .await
        .iter()
        .flat_map(|t| t["legs"].as_array().unwrap().clone())
        .map(|leg| (leg["id"].as_str().unwrap().to_owned(), leg))
        .collect()
}

/// Each account's total of legs.
pub async fn balances(api: &Api, user: &User) -> BTreeMap<String, i64> {
    let mut totals = BTreeMap::new();
    for leg in legs(api, user).await.values() {
        *totals
            .entry(leg["account_id"].as_str().unwrap().to_owned())
            .or_insert(0) += leg["amount_minor"].as_i64().unwrap();
    }
    totals
}

/// The transfer preview, checking the `ETag` header says the same as its tag.
pub async fn preview(api: &Api, user: &User) -> (Value, String) {
    let (status, headers, body) = api
        .exchange("POST", "/api/v1/transfers/preview", &user.token, &[])
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tag = body["tag"].as_str().unwrap().to_owned();
    assert_eq!(headers.get("etag").unwrap().to_str().unwrap(), tag);
    (body, tag)
}

pub async fn apply(api: &Api, user: &User, tag: Option<&str>) -> (StatusCode, Value) {
    let headers: Vec<(&str, &str)> = tag.map(|t| ("if-match", t)).into_iter().collect();
    let (status, _, body) = api
        .exchange("POST", "/api/v1/transfers/apply", &user.token, &headers)
        .await;
    (status, body)
}
