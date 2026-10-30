//! The first account: on a fresh install the first person makes the owner account from the web, and then sign-up
//! closes; later people need a setup link from someone inside.

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use coinroost_server::auth;
use common::Api;
use serde_json::{Value, json};
use sqlx::PgPool;

const PASSWORD: &str = "a long enough password";

async fn call(api: &Api, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    api.call(method, path, None, body).await
}

async fn open(api: &Api) -> bool {
    let (status, body) = call(api, "GET", "/api/v1/first-account", None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["open"].as_bool().unwrap()
}

/// A first-account link's token, asked for the way the web asks.
async fn start(api: &Api) -> (StatusCode, Value) {
    call(api, "POST", "/api/v1/first-account", None).await
}

async fn token(api: &Api) -> String {
    let (status, body) = start(api).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["token"].as_str().unwrap().to_owned()
}

async fn enroll(api: &Api, token: &str, email: &str) -> Value {
    let body = json!({"token": token, "email": email});
    let (status, body) = call(api, "POST", "/api/v1/invites/enrollment", Some(body)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

fn code(enrollment: &Value) -> String {
    let totp = totp_rs::Totp::from_url(enrollment["uri"].as_str().unwrap()).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    totp.generate(now).to_string()
}

async fn accept(api: &Api, token: &str, email: &str, code: &str) -> (StatusCode, Value) {
    let body = json!({"token": token, "email": email, "password": PASSWORD, "code": code});
    call(api, "POST", "/api/v1/invites/accept", Some(body)).await
}

async fn users(api: &Api) -> i64 {
    sqlx::query_scalar("select count(*) from users")
        .fetch_one(&api.db)
        .await
        .unwrap()
}

#[sqlx::test]
async fn the_first_account_is_made_from_the_web_then_sign_up_is_closed(db: PgPool) {
    let api = Api::with(db, true);
    assert!(open(&api).await);
    let token = token(&api).await;
    let check = json!({"token": token});
    let (status, state) = call(&api, "POST", "/api/v1/invites/check", Some(check)).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["new_household"], true);
    assert_eq!(state["first_account"], true);
    let enrollment = enroll(&api, &token, "owner@example.com").await;
    let (status, session) = accept(&api, &token, "owner@example.com", &code(&enrollment)).await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    assert!(!open(&api).await);
    let (status, problem) = start(&api).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(users(&api).await, 1);
}

#[sqlx::test]
async fn a_first_account_link_is_closed_once_any_user_exists(db: PgPool) {
    let api = Api::with(db, true);
    let token = token(&api).await;
    let enrollment = enroll(&api, &token, "late@example.com").await;
    // Someone else becomes a user another way (the console) while this link is still open.
    let household = auth::create_household(&api.db, "Household").await.unwrap();
    auth::create_user(&api.db, household, "console@example.com", PASSWORD)
        .await
        .unwrap();
    for (what, body) in [
        ("check", json!({"token": token})),
        ("enrollment", json!({"token": token, "email": "late@example.com"})),
        (
            "accept",
            json!({"token": token, "email": "late@example.com", "password": PASSWORD,
                "code": code(&enrollment)}),
        ),
    ] {
        let (status, problem) = call(&api, "POST", &format!("/api/v1/invites/{what}"), Some(body)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {problem}");
    }
    assert_eq!(users(&api).await, 1);
}

#[sqlx::test]
async fn concurrent_first_accounts_make_one_owner(db: PgPool) {
    let api = Api::with(db.clone(), true);
    let mut racers = Vec::new();
    for email in ["first@example.com", "second@example.com"] {
        let token = token(&api).await;
        let now = code(&enroll(&api, &token, email).await);
        racers.push((token, email, now));
    }
    // Both get past their checks or wait at the lock; none can write a user until this is released.
    let mut hold = db.begin().await.unwrap();
    sqlx::query("lock table users in exclusive mode")
        .execute(&mut *hold)
        .await
        .unwrap();
    let racers: Vec<_> = racers
        .into_iter()
        .map(|(token, email, now)| {
            let api = Api::with(db.clone(), true);
            tokio::spawn(async move { accept(&api, &token, email, &now).await })
        })
        .collect();
    let mut waiting = 0;
    for _ in 0..400 {
        waiting = sqlx::query_scalar::<_, i64>(
            "select count(*) from pg_stat_activity where datname = current_database() and wait_event_type = 'Lock'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        if waiting == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(waiting, 2, "the two sign-ups never waited together");
    hold.rollback().await.unwrap();
    let mut statuses = Vec::new();
    for racer in racers {
        statuses.push(racer.await.unwrap().0);
    }
    statuses.sort();
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::NOT_FOUND]);
    assert_eq!(users(&api).await, 1);
}

#[sqlx::test]
async fn sign_up_is_closed_unless_the_deployment_opens_it(db: PgPool) {
    let api = Api::new(db);
    assert!(!open(&api).await);
    let (status, problem) = start(&api).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let made: i64 = sqlx::query_scalar("select count(*) from invites")
        .fetch_one(&api.db)
        .await
        .unwrap();
    assert_eq!(made, 0);
}
