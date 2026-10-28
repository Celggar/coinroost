//! Applying the rules again: only through a preview and an apply that carries the preview's tag, and never over
//! a category or payee a person set.

mod common;

use axum::http::StatusCode;
use common::Api;
use common::sorting::*;
use serde_json::{Value, json};
use sqlx::PgPool;

#[sqlx::test]
async fn preview_then_apply_with_the_same_etag(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Apply").await;
    api.import_bac(&user).await;
    let (groceries, people, _) = groceries_and_people(&api, &user).await;
    assert!(
        transactions(&api, &user, "")
            .await
            .iter()
            .all(|t| t["category_id"].is_null())
    );

    let (changes, tag) = preview(&api, &user).await;
    assert_eq!(changes.len(), 3);
    assert_eq!(
        preview(&api, &user).await.1,
        tag,
        "the same changes give the same tag"
    );
    let wallet = changes
        .iter()
        .find(|c| c["description"] == "YAPPY A JOHN DOE")
        .unwrap();
    assert_eq!(wallet["after"]["new_payee"], "JOHN DOE");
    assert_eq!(wallet["before"]["category_id"], Value::Null);
    assert!(
        transactions(&api, &user, "")
            .await
            .iter()
            .all(|t| t["category_id"].is_null()),
        "preview wrote"
    );

    let (status, applied) = apply(&api, &user, Some(&tag)).await;
    assert_eq!(
        (status, &applied["applied"]),
        (StatusCode::OK, &json!(3)),
        "{applied}"
    );
    assert_eq!(preview(&api, &user).await.0, Vec::<Value>::new());
    let items = transactions(&api, &user, "").await;
    assert!(
        by_description(&items, "SUPERMERCADO")
            .iter()
            .all(|t| t["category_id"] == groceries.as_str())
    );
    assert_eq!(
        by_description(&items, "YAPPY A")[0]["category_id"],
        people.as_str()
    );
    assert_eq!(
        payee_names(&api, &user)
            .await
            .iter()
            .filter(|n| *n == "JOHN DOE")
            .count(),
        1
    );
}

#[sqlx::test]
async fn apply_refuses_a_stale_preview(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Stale").await;
    api.import_bac(&user).await;
    groceries_and_people(&api, &user).await;
    let (_, tag) = preview(&api, &user).await;
    let income = category(&api, &user, "Interest").await;
    post(
        &api,
        &user,
        "/api/v1/rules",
        json!({ "name": "interest",
        "when": { "description_contains": "INTERESES" }, "then": { "category_id": income } }),
    )
    .await;

    let (status, problem) = apply(&api, &user, Some(&tag)).await;
    assert_eq!(status, StatusCode::PRECONDITION_FAILED, "{problem}");
    let (status, _) = apply(&api, &user, None).await;
    assert_eq!(status, StatusCode::PRECONDITION_REQUIRED);
    assert_eq!(
        transactions(&api, &user, "&uncategorized=true").await.len(),
        9,
        "a refused apply wrote"
    );
    let (_, fresh) = preview(&api, &user).await;
    assert_eq!(apply(&api, &user, Some(&fresh)).await.0, StatusCode::OK);
}

#[sqlx::test]
async fn manual_category_survives_apply(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Manual").await;
    api.import_bac(&user).await;
    let mine = category(&api, &user, "Mine").await;
    let items = transactions(&api, &user, "").await;
    let markets = by_description(&items, "SUPERMERCADO");
    let (kept, cleared) = (id(markets[0]).to_owned(), id(markets[1]).to_owned());
    for (transaction, category) in [(&kept, json!(mine)), (&cleared, json!(mine))] {
        let uri = format!("/api/v1/transactions/{transaction}");
        let (status, body) = api
            .call(
                "PATCH",
                &uri,
                Some(&user.token),
                Some(json!({ "category_id": category })),
            )
            .await;
        assert_eq!(
            (status, &body["category_set_by"]),
            (StatusCode::OK, &json!("manual")),
            "{body}"
        );
    }
    // Cleared by hand: no category, and rules may sort it again.
    let uri = format!("/api/v1/transactions/{cleared}");
    let (_, body) = api
        .call(
            "PATCH",
            &uri,
            Some(&user.token),
            Some(json!({ "category_id": null })),
        )
        .await;
    assert_eq!(
        (&body["category_id"], &body["category_set_by"]),
        (&Value::Null, &Value::Null)
    );

    let (groceries, _, _) = groceries_and_people(&api, &user).await;
    let (changes, tag) = preview(&api, &user).await;
    let changed: Vec<&str> = changes
        .iter()
        .map(|c| c["transaction_id"].as_str().unwrap())
        .collect();
    assert!(
        changed.contains(&cleared.as_str()) && changed.contains(&kept.as_str()),
        "{changes:?}"
    );
    let kept_change = changes
        .iter()
        .find(|c| c["transaction_id"] == kept.as_str())
        .unwrap();
    assert_eq!(
        kept_change["after"]["category_id"],
        mine.as_str(),
        "only its payee may change"
    );
    apply(&api, &user, Some(&tag)).await;

    let items = transactions(&api, &user, "").await;
    let find = |wanted: &str| items.iter().find(|t| t["id"] == wanted).unwrap().clone();
    assert_eq!(
        (&find(&kept)["category_id"], &find(&kept)["category_set_by"]),
        (&json!(mine), &json!("manual"))
    );
    assert_eq!(
        (&find(&cleared)["category_id"], &find(&cleared)["category_set_by"]),
        (&json!(groceries), &json!("rule"))
    );
}

#[sqlx::test]
async fn payee_from_text_upserts_one_payee(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Wallet").await;
    let account = api.import_bac(&user).await;
    for amount in [-700, -900] {
        post(
            &api,
            &user,
            "/api/v1/transactions",
            json!({ "date": "2026-06-02", "description": "PAGO YAPPY A JOHN DOE",
            "legs": [{ "account_id": account, "amount_minor": amount }] }),
        )
        .await;
    }
    post(
        &api,
        &user,
        "/api/v1/rules",
        json!({ "name": "wallet",
        "when": { "description_contains": "YAPPY A" }, "then": { "payee_from_text": true } }),
    )
    .await;
    let (changes, tag) = preview(&api, &user).await;
    assert_eq!(changes.len(), 3);
    assert!(changes.iter().all(|c| c["after"]["new_payee"] == "JOHN DOE"));
    apply(&api, &user, Some(&tag)).await;
    assert_eq!(payee_names(&api, &user).await, ["JOHN DOE"]);
    let items = transactions(&api, &user, "").await;
    let paid = by_description(&items, "YAPPY A");
    assert_eq!(paid.len(), 3);
    assert!(
        paid.iter()
            .all(|t| t["payee_id"] == paid[0]["payee_id"] && t["payee_set_by"] == "rule")
    );
    // A payee-only rule leaves the category alone: still in the queue.
    assert!(paid.iter().all(|t| t["category_id"].is_null()));
}
