//! Helpers for the rule tests: categories, the two starting rules, transaction lists, preview and apply.

use axum::http::StatusCode;
use serde_json::{Value, json};

use super::{Api, User};

pub async fn post(api: &Api, user: &User, uri: &str, body: Value) -> Value {
    let (status, created) = api.call("POST", uri, Some(&user.token), Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "{uri}: {created}");
    created
}

pub fn id(value: &Value) -> &str {
    value["id"].as_str().unwrap()
}

pub async fn category(api: &Api, user: &User, name: &str) -> String {
    id(&post(api, user, "/api/v1/categories", json!({ "name": name })).await).to_owned()
}

/// The rules every test starts from: groceries with a named payee, and people paid through a wallet.
pub async fn groceries_and_people(api: &Api, user: &User) -> (String, String, String) {
    let groceries = category(api, user, "Groceries").await;
    let people = category(api, user, "People").await;
    let market = id(&post(api, user, "/api/v1/payees", json!({ "name": "Market" })).await).to_owned();
    let first = post(
        api,
        user,
        "/api/v1/rules",
        json!({ "name": "market",
        "when": { "description_contains": "SUPERMERCADO", "direction": "out" },
        "then": { "category_id": groceries, "payee_id": market } }),
    )
    .await;
    post(
        api,
        user,
        "/api/v1/rules",
        json!({ "name": "wallet",
        "when": { "description_contains": "YAPPY A", "direction": "out" },
        "then": { "category_id": people, "payee_from_text": true } }),
    )
    .await;
    (groceries, people, id(&first).to_owned())
}

pub async fn transactions(api: &Api, user: &User, query: &str) -> Vec<Value> {
    let uri = format!("/api/v1/transactions?limit=1000{query}");
    let (status, list) = api.call("GET", &uri, Some(&user.token), None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    list["items"].as_array().unwrap().clone()
}

pub fn by_description<'a>(items: &'a [Value], text: &str) -> Vec<&'a Value> {
    items
        .iter()
        .filter(|t| t["description"].as_str().unwrap().contains(text))
        .collect()
}

pub async fn payee_names(api: &Api, user: &User) -> Vec<String> {
    let (_, list) = api.call("GET", "/api/v1/payees", Some(&user.token), None).await;
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

/// The preview's changes and its tag, checking the `ETag` header says the same.
pub async fn preview(api: &Api, user: &User) -> (Vec<Value>, String) {
    let (status, headers, body) = api
        .exchange("POST", "/api/v1/rules/preview", &user.token, &[], None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tag = body["tag"].as_str().unwrap().to_owned();
    assert_eq!(headers.get("etag").unwrap().to_str().unwrap(), tag);
    (body["changes"].as_array().unwrap().clone(), tag)
}

pub async fn apply(api: &Api, user: &User, tag: Option<&str>) -> (StatusCode, Value) {
    let headers: Vec<(&str, &str)> = tag.map(|t| ("if-match", t)).into_iter().collect();
    let (status, _, body) = api
        .exchange("POST", "/api/v1/rules/apply", &user.token, &headers, None)
        .await;
    (status, body)
}

/// What a person sees of each transaction: category, payee and who set them.
pub fn sorting(items: &[Value]) -> Vec<(Value, Value, Value, Value, Value)> {
    let mut rows: Vec<_> = items
        .iter()
        .map(|t| {
            (
                t["id"].clone(),
                t["category_id"].clone(),
                t["category_set_by"].clone(),
                t["payee_id"].clone(),
                t["payee_set_by"].clone(),
            )
        })
        .collect();
    rows.sort_by_key(|r| r.0.to_string());
    rows
}
