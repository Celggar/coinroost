//! What each employer owes, worked by hand on synthetic deposits.

mod common;

use axum::http::StatusCode;
use coinroost_server::employers::{self, EmployersFile, LoadReport};
use common::sorting::{id, post};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

async fn account(api: &Api, user: &User, name: &str) -> String {
    let body =
        json!({ "name": name, "kind": "checking", "currency": "USD", "namespace": name.to_lowercase() });
    id(&post(api, user, "/api/v1/accounts", body).await).to_owned()
}

async fn money(api: &Api, user: &User, date: &str, account: &str, cents: i64, description: &str) {
    let body = json!({ "date": date, "description": description,
                       "legs": [{ "account_id": account, "amount_minor": cents }] });
    post(api, user, "/api/v1/transactions", body).await;
}

async fn household(api: &Api, user: &User) -> Uuid {
    let (_, me) = api.call("GET", "/api/v1/me", Some(&user.token), None).await;
    me["household_id"].as_str().unwrap().parse().unwrap()
}

const EMPLOYERS: &str = r#"{ "employers": [
  { "name": "Acme", "match": "ACME", "account": "checking", "min_amount_minor": 70000, "amount_minor": 80000,
    "start": "2026-01" },
  { "name": "Nobody", "match": "NOBODY", "account": "checking", "min_amount_minor": 100, "amount_minor": 100,
    "start": "2026-01" } ] }"#;

/// Acme's deposits: January's I on time, January's II named by the bank and a day late, a 13th-month bonus under
/// the minimum, a deposit into another account, and an unnamed payment on 3 March (nearest to February's II).
async fn acme(api: &Api, user: &User) -> String {
    let checking = account(api, user, "Checking").await;
    let savings = account(api, user, "Savings").await;
    money(api, user, "2026-01-15", &checking, 80000, "ACME PLANILLA").await;
    money(api, user, "2026-02-02", &checking, 80000, "ACME II QNA DE ENERO").await;
    money(api, user, "2026-02-20", &checking, 56000, "ACME XIII MES").await;
    money(api, user, "2026-02-16", &savings, 80000, "ACME PAGO").await;
    money(api, user, "2026-03-03", &checking, 80000, "ACME PAGO").await;
    employers::load(
        &api.db,
        household(api, user).await,
        &EmployersFile::from_json(EMPLOYERS).unwrap(),
    )
    .await
    .unwrap();
    checking
}

async fn payroll(api: &Api, user: &User, as_of: &str) -> Vec<Value> {
    let (status, body) = api
        .call(
            "GET",
            &format!("/api/v1/payroll?as_of={as_of}"),
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["employers"].as_array().unwrap().clone()
}

fn fortnights(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|f| format!("{}-{:02}-{}", f["year"], f["month"].as_u64().unwrap(), f["half"]))
        .collect()
}

#[sqlx::test]
async fn payroll_reports_what_each_employer_owes(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Owed").await;
    let checking = acme(&api, &user).await;
    money(&api, &user, "2026-03-20", &checking, -1000, "COFFEE").await;
    let found = payroll(&api, &user, "2026-04-10").await;
    let acme = found.iter().find(|e| e["name"] == "Acme").unwrap();
    // Worked by hand. Due by 10 April: Jan I, Jan II, Feb I, Feb II, Mar I, Mar II; the account's last data is
    // 20 March. Jan I paid the 15th (due the 16th: a day early); Jan II named by the bank, paid 2 Feb (due the 1st:
    // a day late); the 3 March payment settles the newest free one back from Feb II, Feb II itself (2 days late);
    // Feb I stays owed (32 days at 20 March) and Mar I too (4 days); Mar II is due 1 April, after the last data.
    assert_eq!(fortnights(&acme["owed"]), ["2026-02-1", "2026-03-1"]);
    assert_eq!(acme["owed_total_minor"], 160_000);
    assert_eq!(fortnights(&acme["unknown"]), ["2026-03-2"]);
    assert_eq!(
        (&acme["paid"], &acme["data_until"]),
        (&json!(3), &json!("2026-03-20"))
    );
    let lines: Vec<(&str, i64, Option<&str>)> = acme["history"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["status"].as_str().unwrap(),
                l["days"].as_i64().unwrap_or(i64::MIN),
                l["payment"]["description"].as_str(),
            )
        })
        .collect();
    assert_eq!(
        lines,
        [
            ("on_time", -1, Some("ACME PLANILLA")),
            ("late", 1, Some("ACME II QNA DE ENERO")),
            ("owed", 32, None),
            ("late", 2, Some("ACME PAGO")),
            ("owed", 4, None),
            ("no_data", i64::MIN, None)
        ]
    );
    assert_eq!(
        acme["history"][1]["label"]["half"], 2,
        "the fortnight the bank named"
    );
    assert_eq!(fortnights(&json!([acme["next_due"]])), ["2026-04-1"]);
}

#[sqlx::test]
async fn employer_without_deposits_is_an_error(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Nothing").await;
    acme(&api, &user).await;
    let found = payroll(&api, &user, "2026-04-10").await;
    let nobody = found.iter().find(|e| e["name"] == "Nobody").unwrap();
    assert!(
        nobody["error"].as_str().unwrap().contains("no deposit counted"),
        "{nobody}"
    );
    assert_eq!(
        (&nobody["owed"], &nobody["owed_total_minor"]),
        (&json!([]), &json!(0)),
        "never owes everything"
    );
    let acme = found.iter().find(|e| e["name"] == "Acme").unwrap();
    assert_eq!(
        acme["error"],
        Value::Null,
        "one employer's error leaves the others alone"
    );
}

#[sqlx::test]
async fn payroll_waits_for_data_it_does_not_have(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Waiting").await;
    let checking = acme(&api, &user).await;
    // The last data is the 3 March payment: Mar I (due the 16th) is not known yet, even on 10 April.
    let before = payroll(&api, &user, "2026-04-10").await;
    let acme = before.iter().find(|e| e["name"] == "Acme").unwrap();
    assert_eq!(fortnights(&acme["owed"]), ["2026-02-1"]);
    assert_eq!(fortnights(&acme["unknown"]), ["2026-03-1", "2026-03-2"]);
    // A later statement arrives with no payment in it: now Mar I is owed.
    money(&api, &user, "2026-03-20", &checking, -1000, "COFFEE").await;
    let after = payroll(&api, &user, "2026-04-10").await;
    let acme = after.iter().find(|e| e["name"] == "Acme").unwrap();
    assert_eq!(fortnights(&acme["owed"]), ["2026-02-1", "2026-03-1"]);
}

#[sqlx::test]
async fn payroll_load_twice_changes_nothing(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Loaded").await;
    account(&api, &user, "Checking").await;
    let household = household(&api, &user).await;
    let file = EmployersFile::from_json(EMPLOYERS).unwrap();
    assert_eq!(
        employers::load(&db, household, &file).await.unwrap(),
        LoadReport { added: 2, changed: 0 }
    );
    assert_eq!(
        employers::load(&db, household, &file).await.unwrap(),
        LoadReport::default()
    );
    let raised = EmployersFile::from_json(&EMPLOYERS.replace("80000", "85000")).unwrap();
    assert_eq!(
        employers::load(&db, household, &raised).await.unwrap(),
        LoadReport { added: 0, changed: 1 }
    );
    let found = payroll(&api, &user, "2026-04-10").await;
    assert_eq!(
        found.iter().find(|e| e["name"] == "Acme").unwrap()["amount_minor"],
        85000
    );
    let nowhere = EmployersFile::from_json(&EMPLOYERS.replace("\"checking\"", "\"nowhere\"")).unwrap();
    assert!(employers::load(&db, household, &nowhere).await.is_err());
    let undated = EmployersFile::from_json(&EMPLOYERS.replace("2026-01", "January")).unwrap();
    assert!(employers::load(&db, household, &undated).await.is_err());
    // A failed load changes nothing either.
    assert_eq!(
        employers::load(&db, household, &raised).await.unwrap(),
        LoadReport::default()
    );
}
