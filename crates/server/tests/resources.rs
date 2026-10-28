//! Creating and listing each resource, and what the API refuses.

mod common;

use axum::http::StatusCode;
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn account(api: &Api, user: &User, name: &str, namespace: &str) -> Value {
    let body = json!({ "name": name, "kind": "checking", "currency": "usd", "namespace": namespace, "profile": null });
    let (status, account) = api
        .call("POST", "/api/v1/accounts", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    account
}

#[sqlx::test]
async fn creates_and_lists_accounts(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Accounts").await;
    let created = account(&api, &user, "Savings", "savings").await;
    assert_eq!(created["currency"], "USD");
    account(&api, &user, "Card", "card").await;
    let (status, list) = api.call("GET", "/api/v1/accounts", Some(&user.token), None).await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Card", "Savings"]);

    let bad = |change: Value| {
        let mut body = json!({ "name": "X", "kind": "checking", "currency": "USD", "namespace": "x" });
        body.as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        body
    };
    for (body, expected) in [
        (
            bad(json!({ "currency": "XYZ" })),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            bad(json!({ "namespace": "Has Spaces" })),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (bad(json!({ "name": "   " })), StatusCode::UNPROCESSABLE_ENTITY),
        (
            bad(json!({ "profile": "no-such-bank" })),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            bad(json!({ "currency": "EUR", "profile": "bac-account" })),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (bad(json!({ "namespace": "savings" })), StatusCode::CONFLICT),
    ] {
        let (status, problem) = api
            .call("POST", "/api/v1/accounts", Some(&user.token), Some(body.clone()))
            .await;
        assert_eq!(status, expected, "{body} → {problem}");
    }
}

#[sqlx::test]
async fn creates_and_lists_categories(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Categories").await;
    for name in ["Groceries", " Fuel "] {
        let (status, _) = api
            .call(
                "POST",
                "/api/v1/categories",
                Some(&user.token),
                Some(json!({ "name": name })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, _) = api
        .call(
            "POST",
            "/api/v1/categories",
            Some(&user.token),
            Some(json!({ "name": "Fuel" })),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (_, list) = api
        .call("GET", "/api/v1/categories", Some(&user.token), None)
        .await;
    let names: Vec<&str> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Fuel", "Groceries"]);
}

#[sqlx::test]
async fn creates_and_lists_payees(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Payees").await;
    let (status, payee) = api
        .call(
            "POST",
            "/api/v1/payees",
            Some(&user.token),
            Some(json!({ "name": "Corner Shop" })),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, list) = api.call("GET", "/api/v1/payees", Some(&user.token), None).await;
    assert_eq!(list["items"][0]["id"], payee["id"]);
    let (status, _) = api
        .call(
            "POST",
            "/api/v1/payees",
            Some(&user.token),
            Some(json!({ "name": "" })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn creates_and_lists_rules(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Rules").await;
    let (_, category) = api
        .call(
            "POST",
            "/api/v1/categories",
            Some(&user.token),
            Some(json!({ "name": "Fuel" })),
        )
        .await;
    let rule = |name: &str, position: Option<i32>| {
        json!({ "name": name, "position": position, "when": { "description_contains": "station" },
                "then": { "category_id": category["id"] } })
    };
    for (name, position) in [("first", None), ("second", None), ("now first", Some(1))] {
        let (status, body) = api
            .call(
                "POST",
                "/api/v1/rules",
                Some(&user.token),
                Some(rule(name, position)),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
    }
    let (_, list) = api.call("GET", "/api/v1/rules", Some(&user.token), None).await;
    let order: Vec<(i64, &str)> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["position"].as_i64().unwrap(), r["name"].as_str().unwrap()))
        .collect();
    assert_eq!(order, [(1, "now first"), (2, "first"), (3, "second")]);
    assert_eq!(list["items"][0]["when"]["description_contains"], "station");

    for body in [
        json!({ "name": "no condition", "when": {}, "then": { "category_id": category["id"] } }),
        json!({ "name": "no action", "when": { "direction": "out" }, "then": {} }),
        json!({ "name": "range", "when": { "amount_min": 5, "amount_max": 1 }, "then": { "category_id": category["id"] } }),
        json!({ "name": "far", "position": 9, "when": { "direction": "in" }, "then": { "category_id": category["id"] } }),
        json!({ "name": "unknown", "when": { "direction": "in" },
                "then": { "category_id": "00000000-0000-0000-0000-000000000009" } }),
    ] {
        let (status, problem) = api
            .call("POST", "/api/v1/rules", Some(&user.token), Some(body.clone()))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body} → {problem}");
    }
}

#[sqlx::test]
async fn creates_and_lists_transactions(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Spending").await;
    let checking = account(&api, &user, "Checking", "checking").await;
    let body = json!({ "date": "2026-06-05", "description": "Groceries",
                       "legs": [{ "account_id": checking["id"], "amount_minor": -4599 }] });
    let (status, created) = api
        .call("POST", "/api/v1/transactions", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["legs"][0]["currency"], "USD");
    assert_eq!(created["legs"][0]["posted_on"], "2026-06-05");
    let uri = format!(
        "/api/v1/transactions?account_id={}&from=2026-06-01&to=2026-06-30",
        checking["id"].as_str().unwrap()
    );
    let (_, list) = api.call("GET", &uri, Some(&user.token), None).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1);
    assert_eq!(list["items"][0]["legs"][0]["amount_minor"], -4599);
    let (_, outside) = api
        .call(
            "GET",
            "/api/v1/transactions?from=2026-07-01",
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(outside["items"], json!([]));
}

#[sqlx::test]
async fn creates_and_lists_transfers(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Transfers").await;
    let checking = account(&api, &user, "Checking", "checking").await;
    let savings = account(&api, &user, "Savings", "savings").await;
    let transfer = |a: i64, b: i64, to: &Value| {
        json!({ "date": "2026-06-20", "description": "To savings",
                "legs": [{ "account_id": checking["id"], "amount_minor": a }, { "account_id": to["id"], "amount_minor": b }] })
    };
    let (status, created) = api
        .call(
            "POST",
            "/api/v1/transactions",
            Some(&user.token),
            Some(transfer(-10000, 10000, &savings)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["legs"].as_array().unwrap().len(), 2);
    // The transfer shows on both accounts.
    for side in [&checking, &savings] {
        let uri = format!("/api/v1/transactions?account_id={}", side["id"].as_str().unwrap());
        let (_, list) = api.call("GET", &uri, Some(&user.token), None).await;
        assert_eq!(list["items"][0]["id"], created["id"]);
    }
    for body in [
        transfer(-10000, 9999, &savings),
        transfer(-1, 1, &checking),
        transfer(0, 0, &savings),
    ] {
        let (status, problem) = api
            .call("POST", "/api/v1/transactions", Some(&user.token), Some(body))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    }
}

/// One household never sees, lists or points at another's rows, whatever ids it sends.
#[sqlx::test]
async fn households_cannot_see_each_other(db: PgPool) {
    let api = Api::new(db);
    let a = api.household("Alpha").await;
    let b = api.household("Beta").await;
    let a_account = account(&api, &a, "Checking", "checking").await;
    let (_, a_category) = api
        .call(
            "POST",
            "/api/v1/categories",
            Some(&a.token),
            Some(json!({ "name": "Fuel" })),
        )
        .await;
    api.call(
        "POST",
        "/api/v1/payees",
        Some(&a.token),
        Some(json!({ "name": "Shop" })),
    )
    .await;
    let rule =
        json!({ "name": "r", "when": { "direction": "out" }, "then": { "category_id": a_category["id"] } });
    api.call("POST", "/api/v1/rules", Some(&a.token), Some(rule.clone()))
        .await;
    let spend = json!({ "date": "2026-06-05", "description": "x", "legs": [{ "account_id": a_account["id"], "amount_minor": -1 }] });
    api.call(
        "POST",
        "/api/v1/transactions",
        Some(&a.token),
        Some(spend.clone()),
    )
    .await;

    for list in [
        "/api/v1/accounts",
        "/api/v1/categories",
        "/api/v1/payees",
        "/api/v1/rules",
        "/api/v1/transactions",
        "/api/v1/imports",
    ] {
        let (status, body) = api.call("GET", list, Some(&b.token), None).await;
        assert_eq!((status, &body["items"]), (StatusCode::OK, &json!([])), "{list}");
    }
    let by_account = format!(
        "/api/v1/transactions?account_id={}",
        a_account["id"].as_str().unwrap()
    );
    let (_, body) = api.call("GET", &by_account, Some(&b.token), None).await;
    assert_eq!(body["items"], json!([]));

    // B cannot write into A's account or use A's category; the same name is free in B's household.
    let (status, _) = api
        .call("POST", "/api/v1/transactions", Some(&b.token), Some(spend))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = api
        .call("POST", "/api/v1/rules", Some(&b.token), Some(rule))
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let import = format!(
        "/api/v1/accounts/{}/imports?file_name=x.csv",
        a_account["id"].as_str().unwrap()
    );
    let (status, _) = api.call("POST", &import, Some(&b.token), Some(json!("x"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    account(&api, &b, "Checking", "checking").await;
}
