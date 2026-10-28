//! `firefly import` and `firefly check` on a snapshot of the test Firefly: every month-end balance matches
//! Firefly's hand-worked figure, the legs follow the accounts, and nothing is added twice whichever comes first,
//! the migration or a statement.

mod common;

use std::path::PathBuf;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use coinroost_server::firefly::plan::Map;
use coinroost_server::firefly::snapshot::Snapshot;
use coinroost_server::firefly::{check, fetch, import};
use common::firefly::{TOKEN, TestFirefly, fixture, scratch};
use common::{Api, User};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

async fn snapshot() -> Snapshot {
    let firefly = TestFirefly::start().await;
    let out: PathBuf = scratch().join("snapshot");
    fetch::fetch(&firefly.url, TOKEN, &out).await.unwrap();
    Snapshot::load(&out).unwrap()
}

fn map() -> Map {
    serde_json::from_str(&std::fs::read_to_string(fixture().join("map.json")).unwrap()).unwrap()
}

async fn household(api: &Api, user: &User) -> Uuid {
    let (_, me) = api.call("GET", "/api/v1/me", Some(&user.token), None).await;
    me["household_id"].as_str().unwrap().parse().unwrap()
}

/// Imports the synthetic BAC statement into the account with that namespace; returns the import's report.
async fn statement(api: &Api, user: &User, account: Uuid) -> Value {
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/accounts/{account}/imports?file_name=statement.csv"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", user.token))
        .body(Body::from(
            include_bytes!("../../../spec/fixtures/bac-account/input.csv").to_vec(),
        ))
        .unwrap();
    let (status, report) = api.send(request).await;
    assert_eq!(status, StatusCode::CREATED, "{report}");
    report
}

async fn account(db: &PgPool, namespace: &str) -> Uuid {
    let (id,): (Uuid,) = sqlx::query_as("select id from accounts where namespace = $1")
        .bind(namespace)
        .fetch_one(db)
        .await
        .unwrap();
    id
}

async fn count(db: &PgPool, table: &str) -> i64 {
    let sql = match table {
        "legs" => "select count(*) from legs",
        _ => "select count(*) from transactions",
    };
    sqlx::query_scalar(sql).fetch_one(db).await.unwrap()
}

#[sqlx::test]
async fn month_end_balances_match_firefly_after_the_import(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Migrated").await;
    let (household, snapshot) = (household(&api, &user).await, snapshot().await);
    let report = import::import(&db, household, &snapshot, &map()).await.unwrap();
    assert!(
        report.contains("journals 25: 25 added, 0 partly present"),
        "{report}"
    );
    assert!(report.contains("accounts 5 (5 created)"), "{report}");
    let line = |kind: &str| report.lines().find(|l| l.trim_start().starts_with(kind)).unwrap();
    for gap in ["tags", "budgets", "bills", "piggy-banks", "rules"] {
        assert!(line(gap).contains("GAP"), "{}", line(gap));
    }
    assert!(
        line("webhooks").contains("off in this Firefly"),
        "{}",
        line("webhooks")
    );
    assert!(
        report.contains("tags on 2, budgets on 1, bills on 1, notes on 2"),
        "{report}"
    );

    let (lines, differ) = check::check(&db, household, &snapshot, &map()).await.unwrap();
    assert_eq!(differ, 0, "{lines}");
    assert!(lines.contains("15 month ends compared"), "{lines}");
}

/// Each leg's account, amount and external id, ordered by account name.
async fn legs(db: &PgPool, description: &str) -> Vec<(String, i64, Option<String>)> {
    sqlx::query_as(
        "select a.name, l.amount_minor, l.external_id from legs l join transactions t on t.id = l.transaction_id
         join accounts a on a.id = l.account_id where t.description = $1 order by t.date, a.name",
    )
    .bind(description)
    .fetch_all(db)
    .await
    .unwrap()
}

#[sqlx::test]
async fn the_legs_follow_the_accounts_not_firefly_types(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Legs").await;
    import::import(&db, household(&api, &user).await, &snapshot().await, &map())
        .await
        .unwrap();
    let names = |l: Vec<(String, i64, Option<String>)>| -> Vec<(String, i64)> {
        l.into_iter().map(|(a, m, _)| (a, m)).collect()
    };
    // A withdrawal between two own accounts is a transfer; a transfer into yen takes the foreign amount.
    let paid = names(legs(&db, "Card payment").await);
    assert_eq!(
        paid,
        [
            ("BAC Checking".to_owned(), -5000),
            ("Store card".to_owned(), 5000)
        ]
    );
    let yen = names(legs(&db, "To the yen wallet").await);
    assert_eq!(
        yen,
        [
            ("BAC Checking".to_owned(), -1000),
            ("Yen wallet".to_owned(), 1500)
        ]
    );
    assert_eq!(
        names(legs(&db, "Opening debt").await),
        [("Store card".to_owned(), -20000)]
    );
    // The hub's merged transfer: the inflow's id came from the notes.
    let moved = legs(&db, "TRANSF A CUENTA PROPIA").await;
    let may = &moved[moved.len() - 2..];
    assert_eq!(
        may[1].2.as_deref(),
        Some(format!("f{:031x}", 0x16).as_str()),
        "{may:?}"
    );
    assert_eq!(may[0].2.as_deref(), Some("1b9ececc7734787c4a9e5cd62de50907"));
}

#[sqlx::test]
async fn categories_and_payees_keep_what_firefly_had(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Sorting").await;
    import::import(&db, household(&api, &user).await, &snapshot().await, &map())
        .await
        .unwrap();
    let (date, payee, set_by): (time::Date, String, String) = sqlx::query_as(
        "select t.date, p.name, t.category_set_by from transactions t join payees p on p.id = t.payee_id
         where t.description = 'Late coffee' or t.description = 'DEVOLUCION COMPRA' order by t.date",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(
        (date.to_string(), payee.as_str(), set_by.as_str()),
        ("2026-03-31".to_owned(), "Cafe Ejemplo", "manual")
    );
    let kinds: Vec<(String, String)> = sqlx::query_as("select name, kind from categories order by name")
        .fetch_all(&db)
        .await
        .unwrap();
    let kind = |n: &str| kinds.iter().find(|(name, _)| name == n).unwrap().1.clone();
    assert_eq!(
        [
            kind("Own transfers"),
            kind("Salary"),
            kind("Groceries"),
            kind("Fees")
        ],
        ["transfer", "income", "expense", "expense"]
    );
    let markets: i64 = sqlx::query_scalar("select count(*) from payees where name = 'Supermercado Ejemplo'")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(
        markets, 1,
        "an expense and a revenue account of one name are one payee"
    );
}

#[sqlx::test]
async fn importing_twice_adds_nothing(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Twice").await;
    let (household, snapshot) = (household(&api, &user).await, snapshot().await);
    import::import(&db, household, &snapshot, &map()).await.unwrap();
    let (transactions, legs) = (count(&db, "transactions").await, count(&db, "legs").await);
    let again = import::import(&db, household, &snapshot, &map()).await.unwrap();
    assert!(
        again.contains(
            "0 added, 0 partly present (missing legs added), 0 already from a statement, 25 migrated before"
        ),
        "{again}"
    );
    assert_eq!(
        (count(&db, "transactions").await, count(&db, "legs").await),
        (transactions, legs)
    );
}

#[sqlx::test]
async fn migration_then_statement_adds_nothing(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Migration first").await;
    import::import(&db, household(&api, &user).await, &snapshot().await, &map())
        .await
        .unwrap();
    let report = statement(&api, &user, account(&db, "bac").await).await;
    assert_eq!(
        (&report["batch"]["added"], &report["batch"]["duplicates"]),
        (&Value::from(0), &Value::from(9)),
        "{report}"
    );
}

#[sqlx::test]
async fn statement_then_migration_adds_only_the_rest(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Statement first").await;
    let household = household(&api, &user).await;
    api.import_bac(&user).await;
    let snapshot = snapshot().await;
    let report = import::import(&db, household, &snapshot, &map()).await.unwrap();
    // Two of May's nine are transfers whose other leg the statement could not know: those legs are added.
    assert!(
        report.contains("16 added, 2 partly present (missing legs added), 7 already from a statement"),
        "{report}"
    );
    let (lines, differ) = check::check(&db, household, &snapshot, &map()).await.unwrap();
    assert_eq!(differ, 0, "{lines}");
}

#[sqlx::test]
async fn an_account_missing_from_the_map_stops_the_import(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Unmapped").await;
    let (household, snapshot) = (household(&api, &user).await, snapshot().await);
    let mut partial = map();
    partial.accounts.remove("5");
    let refused = import::import(&db, household, &snapshot, &partial)
        .await
        .unwrap_err();
    assert!(refused.contains("5 (Old savings)"), "{refused}");
    let mut renamed = map();
    renamed.accounts.get_mut("2").unwrap().name = "Savings account".to_owned();
    assert!(
        import::import(&db, household, &snapshot, &renamed)
            .await
            .unwrap_err()
            .contains("\"Savings\"")
    );
    assert_eq!(count(&db, "transactions").await, 0);
}
