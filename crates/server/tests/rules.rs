//! Sorting transactions with rules at import, and what changing the rules does (nothing to transactions).

mod common;

use axum::http::StatusCode;
use coinroost_rules::RuleSet;
use coinroost_server::rule_set;
use common::Api;
use common::sorting::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn import_categorizes_new_transactions(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Sorted").await;
    let (groceries, people, market_rule) = groceries_and_people(&api, &user).await;
    api.import_bac(&user).await;
    let items = transactions(&api, &user, "").await;
    for market in by_description(&items, "SUPERMERCADO") {
        assert_eq!(market["category_id"], groceries.as_str());
        assert_eq!(
            (&market["category_set_by"], &market["payee_set_by"]),
            (&json!("rule"), &json!("rule"))
        );
        assert_eq!(market["rule_id"], market_rule.as_str());
    }
    let wallet = by_description(&items, "YAPPY A")[0];
    assert_eq!(wallet["category_id"], people.as_str());
    assert!(payee_names(&api, &user).await.contains(&"JOHN DOE".to_owned()));
    let others: Vec<_> = items.iter().filter(|t| t["category_id"].is_null()).collect();
    assert_eq!(others.len(), 6);
    assert!(
        others
            .iter()
            .all(|t| t["category_set_by"].is_null() && t["rule_id"].is_null())
    );
}

#[sqlx::test]
async fn rule_changes_touch_no_transaction(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Stable").await;
    let (_, people, market_rule) = groceries_and_people(&api, &user).await;
    api.import_bac(&user).await;
    let before = sorting(&transactions(&api, &user, "").await);

    let anything = post(
        &api,
        &user,
        "/api/v1/rules",
        json!({ "name": "any purchase", "position": 1,
        "when": { "description_contains": "COMPRA" }, "then": { "category_id": people } }),
    )
    .await;
    let uri = format!("/api/v1/rules/{market_rule}");
    let (status, _) = api
        .call(
            "PATCH",
            &uri,
            Some(&user.token),
            Some(json!({ "then": { "category_id": people }, "position": 3 })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = api
        .call(
            "PATCH",
            &uri,
            Some(&user.token),
            Some(json!({ "when": { "amount_min": 9, "amount_max": 1 } })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = api
        .call(
            "DELETE",
            &format!("/api/v1/rules/{}", id(&anything)),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, rules) = api.call("GET", "/api/v1/rules", Some(&user.token), None).await;
    let order: Vec<(i64, &str)> = rules["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| (r["position"].as_i64().unwrap(), r["name"].as_str().unwrap()))
        .collect();
    assert_eq!(order, [(1, "wallet"), (2, "market")]);

    assert_eq!(sorting(&transactions(&api, &user, "").await), before);
    // The new rules only show as changes waiting to be applied.
    assert_eq!(preview(&api, &user).await.0.len(), 2);
}

#[sqlx::test]
async fn uncategorized_lists_only_uncategorized(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Queue").await;
    groceries_and_people(&api, &user).await;
    api.import_bac(&user).await;
    let left = transactions(&api, &user, "&uncategorized=true").await;
    assert_eq!(left.len(), 6);
    assert!(left.iter().all(|t| t["category_id"].is_null()));
    assert!(by_description(&left, "SUPERMERCADO").is_empty() && by_description(&left, "YAPPY").is_empty());
}

#[sqlx::test]
async fn rule_set_loads_twice_without_changes(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Loaded").await;
    api.import_bac(&user).await;
    let (_, me) = api.call("GET", "/api/v1/me", Some(&user.token), None).await;
    let household = me["household_id"].as_str().unwrap().parse().unwrap();
    let text = r#"{
      "categories": [{ "name": "Groceries", "kind": "expense" }, { "name": "Pay", "kind": "income" }],
      "rules": [
        { "name": "market", "when": { "contains": "SUPERMERCADO ", "direction": "out", "account": "bac" },
          "then": { "category": "Groceries", "payee": "Market" } },
        { "name": "wallet", "when": { "contains": "YAPPY A" }, "then": { "payee_from_text": true } },
        { "name": "salary", "when": { "contains": "ACH CRE", "direction": "in", "amount_min": 100000 },
          "then": { "category": "Pay" } }
      ]
    }"#;
    let set = RuleSet::from_json(text).unwrap();
    let first = rule_set::load(&db, household, &set).await.unwrap();
    assert_eq!(
        (
            first.categories_added,
            first.payees_added,
            first.rules_added,
            first.rules_changed
        ),
        (2, 1, 3, 0)
    );
    let (_, rules) = api.call("GET", "/api/v1/rules", Some(&user.token), None).await;
    assert_eq!(
        rules["items"][0]["when"]["description_contains"], "SUPERMERCADO ",
        "kept as written"
    );
    assert_eq!(
        rule_set::load(&db, household, &set).await.unwrap(),
        rule_set::LoadReport::default()
    );
    let (_, again) = api.call("GET", "/api/v1/rules", Some(&user.token), None).await;
    assert_eq!(again, rules);

    let changed = RuleSet::from_json(&text.replace("\"amount_min\": 100000", "\"amount_min\": 1")).unwrap();
    let second = rule_set::load(&db, household, &changed).await.unwrap();
    assert_eq!((second.rules_added, second.rules_changed), (0, 1));
    let unknown = RuleSet::from_json(&text.replace("\"bac\"", "\"nowhere\"")).unwrap();
    assert!(rule_set::load(&db, household, &unknown).await.is_err());
}
