//! Sorting transactions with rules: at import, and again only through a preview and an apply that carries the
//! preview's tag. A category or payee a person set is never changed by a rule.

mod common;

use axum::http::StatusCode;
use coinroost_rules::RuleSet;
use coinroost_server::rule_set;
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn post(api: &Api, user: &User, uri: &str, body: Value) -> Value {
    let (status, created) = api.call("POST", uri, Some(&user.token), Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "{uri}: {created}");
    created
}

fn id(value: &Value) -> &str {
    value["id"].as_str().unwrap()
}

async fn category(api: &Api, user: &User, name: &str) -> String {
    id(&post(api, user, "/api/v1/categories", json!({ "name": name })).await).to_owned()
}

/// The rules every test starts from: groceries with a named payee, and people paid through a wallet.
async fn groceries_and_people(api: &Api, user: &User) -> (String, String, String) {
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

async fn transactions(api: &Api, user: &User, query: &str) -> Vec<Value> {
    let uri = format!("/api/v1/transactions?limit=1000{query}");
    let (status, list) = api.call("GET", &uri, Some(&user.token), None).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    list["items"].as_array().unwrap().clone()
}

fn by_description<'a>(items: &'a [Value], text: &str) -> Vec<&'a Value> {
    items
        .iter()
        .filter(|t| t["description"].as_str().unwrap().contains(text))
        .collect()
}

async fn payee_names(api: &Api, user: &User) -> Vec<String> {
    let (_, list) = api.call("GET", "/api/v1/payees", Some(&user.token), None).await;
    list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_owned())
        .collect()
}

/// The preview's changes and its tag, checking the `ETag` header says the same.
async fn preview(api: &Api, user: &User) -> (Vec<Value>, String) {
    let (status, headers, body) = api
        .exchange("POST", "/api/v1/rules/preview", &user.token, &[], None)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tag = body["tag"].as_str().unwrap().to_owned();
    assert_eq!(headers.get("etag").unwrap().to_str().unwrap(), tag);
    (body["changes"].as_array().unwrap().clone(), tag)
}

async fn apply(api: &Api, user: &User, tag: Option<&str>) -> (StatusCode, Value) {
    let headers: Vec<(&str, &str)> = tag.map(|t| ("if-match", t)).into_iter().collect();
    let (status, _, body) = api
        .exchange("POST", "/api/v1/rules/apply", &user.token, &headers, None)
        .await;
    (status, body)
}

/// What a person sees of each transaction: category, payee and who set them.
fn sorting(items: &[Value]) -> Vec<(Value, Value, Value, Value, Value)> {
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
