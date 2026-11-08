//! The rule list a page at a time and filtered by text. `total` counts every match and `next_cursor` leads to the
//! rest, so a household with hundreds of rules can be read in parts; without `limit` every match comes at once, as
//! the Rules page reads it.

mod common;

use axum::http::StatusCode;
use common::sorting::{category, id, post};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn get(api: &Api, user: &User, query: &str) -> (StatusCode, Value) {
    api.call("GET", &format!("/api/v1/rules?{query}"), Some(&user.token), None)
        .await
}

/// A rule at the end of the list that looks for `text` and sets `category`.
async fn rule(api: &Api, user: &User, category: &str, name: &str, text: &str) -> String {
    let body = json!({ "name": name, "when": { "description_contains": text }, "then": { "category_id": category } });
    id(&post(api, user, "/api/v1/rules", body).await).to_owned()
}

/// `count` rules named `store 1`, `store 2`… in that order.
async fn stores(api: &Api, user: &User, count: usize) -> Vec<String> {
    let shopping = category(api, user, "Shopping").await;
    let mut made = Vec::new();
    for n in 1..=count {
        let (name, text) = (format!("store {n}"), format!("STORE {n}"));
        made.push(rule(api, user, &shopping, &name, &text).await);
    }
    made
}

fn item_ids(page: &Value) -> Vec<String> {
    let items = page["items"].as_array().unwrap();
    items
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_owned())
        .collect()
}

/// Every page of `query`, following `next_cursor` until it is null.
async fn walk(api: &Api, user: &User, query: &str) -> Vec<Value> {
    let mut pages = Vec::new();
    let mut next = query.to_owned();
    while pages.len() < 20 {
        let (status, page) = get(api, user, &next).await;
        assert_eq!(status, StatusCode::OK, "{next}: {page}");
        let cursor = page["next_cursor"].as_str().map(str::to_owned);
        pages.push(page);
        let Some(cursor) = cursor else { return pages };
        next = format!("{query}&cursor={cursor}");
    }
    panic!("{query}: next_cursor never ends");
}

fn sizes(pages: &[Value]) -> Vec<usize> {
    pages
        .iter()
        .map(|p| p["items"].as_array().unwrap().len())
        .collect()
}

#[sqlx::test]
async fn pages_hold_every_rule_once_in_order(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Rule pages").await;
    let made = stores(&api, &user, 7).await;

    let pages = walk(&api, &user, "limit=3").await;
    assert_eq!(sizes(&pages), [3, 3, 1]);
    for page in &pages {
        assert_eq!(page["total"], 7, "every page counts every rule: {page}");
    }
    let seen: Vec<String> = pages.iter().flat_map(item_ids).collect();
    assert_eq!(seen, made, "every rule once, first one first");
    let positions: Vec<i64> = pages
        .iter()
        .flat_map(|p| p["items"].as_array().unwrap())
        .map(|r| r["position"].as_i64().unwrap())
        .collect();
    assert_eq!(positions, (1..=7).collect::<Vec<i64>>());

    // A page that ends exactly at the last rule says no more follow.
    assert_eq!(sizes(&walk(&api, &user, "limit=7").await), [7]);
}

#[sqlx::test]
async fn without_a_limit_every_rule_comes_with_its_total(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Every rule").await;
    let made = stores(&api, &user, 30).await;
    let (status, whole) = get(&api, &user, "").await;
    assert_eq!(status, StatusCode::OK, "{whole}");
    assert_eq!(item_ids(&whole), made, "no default page size cuts the Rules page");
    assert_eq!(
        (&whole["total"], &whole["next_cursor"]),
        (&json!(30), &Value::Null)
    );
}

#[sqlx::test]
async fn text_finds_rules_by_name_or_by_what_they_look_for(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Rule search").await;
    let food = category(&api, &user, "Food").await;
    let mut made = Vec::new();
    for (name, text) in [
        ("coffee", "CAFE NORTE"),
        ("market", "SUPERMERCADO"),
        ("Cafe by the sea", "MAR AZUL"),
        ("tea and cake", "CAFETERIA"),
        ("salary", "PLANILLA 100%"),
        ("snacks", "kiosk cafe"),
    ] {
        made.push(rule(&api, &user, &food, name, text).await);
    }
    let cafes: Vec<String> = [0, 2, 3, 5].map(|n| made[n].clone()).to_vec();

    let (_, found) = get(&api, &user, "q=Cafe").await;
    assert_eq!(
        item_ids(&found),
        cafes,
        "by name or by text, upper or lower case alike"
    );
    assert_eq!(
        (&found["total"], &found["next_cursor"]),
        (&json!(4), &Value::Null)
    );
    let (_, market) = get(&api, &user, "q=supermercado").await;
    assert_eq!(item_ids(&market), [made[1].clone()]);
    // A percent sign is a character to find, not a pattern.
    let (_, percent) = get(&api, &user, "q=%25").await;
    assert_eq!(item_ids(&percent), [made[4].clone()]);

    let pages = walk(&api, &user, "q=cafe&limit=2").await;
    assert_eq!(
        sizes(&pages),
        [2, 2],
        "4 matches in pages of 2: no empty third page"
    );
    assert!(pages.iter().all(|p| p["total"] == 4), "{pages:?}");
    assert_eq!(pages.iter().flat_map(item_ids).collect::<Vec<_>>(), cafes);
}

#[sqlx::test]
async fn a_rule_deleted_or_added_between_pages_hides_or_repeats_no_other(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Rule edits").await;
    let made = stores(&api, &user, 7).await;

    let (_, first) = get(&api, &user, "limit=3").await;
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    let uri = format!("/api/v1/rules/{}", made[1]);
    let (status, _) = api.call("DELETE", &uri, Some(&user.token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, second) = get(&api, &user, &format!("limit=3&cursor={cursor}")).await;
    assert_eq!(
        item_ids(&second),
        made[3..6],
        "the rules after the page read, though they moved up"
    );

    let (_, first) = get(&api, &user, "limit=3").await;
    assert_eq!(item_ids(&first), [0, 2, 3].map(|n| made[n].clone()));
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    let shopping = category(&api, &user, "More shopping").await;
    let first = json!({ "name": "new first", "position": 1, "when": { "description_contains": "NEW" },
                        "then": { "category_id": shopping } });
    post(&api, &user, "/api/v1/rules", first).await;
    let (_, second) = get(&api, &user, &format!("limit=3&cursor={cursor}")).await;
    assert_eq!(
        item_ids(&second),
        made[4..7],
        "the rules after the page read, though they moved down"
    );
}

#[sqlx::test]
async fn a_cursor_limit_or_text_the_api_cannot_use_is_refused(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Rule refusals").await;
    stores(&api, &user, 2).await;
    // Not base64; base64 of the wrong shape; a transaction list's cursor; a rule cursor with no number.
    for cursor in [
        "not.a.cursor",
        "eC55Lno",
        "MjAyNi0wMi0yOC4wLjAwMDAwMDAwLTAwMDAtMDAwMC0wMDAwLTAwMDAwMDAwMDAwMA",
        "cnVsZS54LjAwMDAwMDAwLTAwMDAtMDAwMC0wMDAwLTAwMDAwMDAwMDAwMA",
    ] {
        let (status, body) = get(&api, &user, &format!("limit=1&cursor={cursor}")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{cursor}: {body}");
        assert_eq!(body["code"], "cursor_invalid", "{cursor}: {body}");
    }
    for limit in ["0", "-1", "201"] {
        let (status, body) = get(&api, &user, &format!("limit={limit}")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{limit}: {body}");
        assert_eq!(body["code"], "limit_out_of_range", "{limit}: {body}");
    }
    let long = "a".repeat(201);
    for text in ["", "%20%20", long.as_str()] {
        let (status, body) = get(&api, &user, &format!("q={text}")).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{text}: {body}");
        assert_eq!(body["code"], "search_length", "{text}: {body}");
    }
}
