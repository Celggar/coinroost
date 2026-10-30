//! Setup links: a person gets their user from the web, once, with the authenticator secret the server made.

mod common;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use coinroost_server::{auth, invites};
use common::Api;
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const PASSWORD: &str = "a long enough password";
const EMAIL: &str = "new@example.com";

async fn link(api: &Api, household: Option<Uuid>) -> String {
    invites::create(&api.db, household).await.unwrap().token
}

async fn post(api: &Api, what: &str, body: Value) -> (StatusCode, Value) {
    api.call("POST", &format!("/api/v1/invites/{what}"), None, Some(body))
        .await
}

async fn enroll(api: &Api, token: &str) -> Value {
    let (status, body) = post(api, "enrollment", json!({"token": token, "email": EMAIL})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

/// The code the enrolled authenticator shows `steps` 30-second steps from now.
fn code(enrollment: &Value, steps: i64) -> String {
    let totp = totp_rs::Totp::from_url(enrollment["uri"].as_str().unwrap()).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    totp.generate(now.checked_add_signed(steps * 30).unwrap())
        .to_string()
}

/// A code that is not the current one (nor the steps around it).
fn wrong(enrollment: &Value) -> String {
    let near: Vec<String> = (-1..=1).map(|s| code(enrollment, s)).collect();
    (0..1_000_000)
        .map(|n| format!("{n:06}"))
        .find(|c| !near.contains(c))
        .unwrap()
}

async fn accept(api: &Api, token: &str, email: &str, code: &str) -> (StatusCode, Value) {
    let body = json!({"token": token, "email": email, "password": PASSWORD, "code": code});
    post(api, "accept", body).await
}

async fn users(api: &Api) -> i64 {
    sqlx::query_scalar("select count(*) from users")
        .fetch_one(&api.db)
        .await
        .unwrap()
}

async fn attempts(api: &Api, token: &str) -> i32 {
    sqlx::query_scalar("select failed_attempts from invites where token_hash = $1")
        .bind(auth::token_hash(token))
        .fetch_one(&api.db)
        .await
        .unwrap()
}

/// The three operations a link allows, each with a body that would succeed on a working link.
async fn all_refused(api: &Api, token: &str, enrollment: &Value) {
    for (what, body) in [
        ("check", json!({"token": token})),
        ("enrollment", json!({"token": token, "email": EMAIL})),
        (
            "accept",
            json!({"token": token, "email": "other@example.com", "password": PASSWORD,
            "code": code(enrollment, 0)}),
        ),
    ] {
        let (status, problem) = post(api, what, body).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{what}: {problem}");
        assert_eq!(problem["status"], 404);
    }
}

#[sqlx::test]
async fn a_link_makes_a_user_who_logs_in_again(db: PgPool) {
    let api = Api::new(db);
    let token = link(&api, None).await;
    let (status, state) = post(&api, "check", json!({"token": token})).await;
    assert_eq!(status, StatusCode::OK, "{state}");
    assert_eq!(state["new_household"], true);
    let enrollment = enroll(&api, &token).await;
    assert!(enrollment["uri"].as_str().unwrap().starts_with("otpauth://totp/"));
    let from_uri = totp_rs::Totp::from_url(enrollment["uri"].as_str().unwrap()).unwrap();
    assert_eq!(enrollment["secret"], from_uri.secret().to_base32());
    assert!(enrollment["qr_svg"].as_str().unwrap().starts_with("<svg"));
    assert_eq!(
        enroll(&api, &token).await["secret"],
        enrollment["secret"],
        "the secret changed between calls"
    );

    let (status, session) = accept(&api, &token, EMAIL, &code(&enrollment, 0)).await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let (status, me) = api
        .call("GET", "/api/v1/me", session["token"].as_str(), None)
        .await;
    assert_eq!((status, me["email"].as_str()), (StatusCode::OK, Some(EMAIL)));

    let login = |code: String| json!({"email": EMAIL, "password": PASSWORD, "code": code});
    let (status, _) = api
        .call(
            "POST",
            "/api/v1/sessions",
            None,
            Some(login(code(&enrollment, 0))),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the code used to accept logged in again"
    );
    let (status, body) = api
        .call(
            "POST",
            "/api/v1/sessions",
            None,
            Some(login(code(&enrollment, 1))),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "the next code does not log in: {body}"
    );
}

#[sqlx::test]
async fn a_used_link_answers_404_everywhere(db: PgPool) {
    let api = Api::new(db);
    let token = link(&api, None).await;
    let enrollment = enroll(&api, &token).await;
    assert_eq!(
        accept(&api, &token, EMAIL, &code(&enrollment, 0)).await.0,
        StatusCode::CREATED
    );
    all_refused(&api, &token, &enrollment).await;
    let used: (bool, bool) = sqlx::query_as("select used_at is not null, used_by is not null from invites")
        .fetch_one(&api.db)
        .await
        .unwrap();
    assert_eq!(used, (true, true));
    assert_eq!(users(&api).await, 1);
}

#[sqlx::test]
async fn an_expired_link_answers_404_everywhere(db: PgPool) {
    let api = Api::new(db);
    let token = link(&api, None).await;
    let enrollment = enroll(&api, &token).await;
    sqlx::query("update invites set expires_at = now() - interval '1 second'")
        .execute(&api.db)
        .await
        .unwrap();
    all_refused(&api, &token, &enrollment).await;
    assert_eq!(users(&api).await, 0);
}

#[sqlx::test]
async fn an_unknown_or_missing_token_is_refused(db: PgPool) {
    let api = Api::new(db);
    let real = link(&api, None).await;
    let enrollment = enroll(&api, &real).await;
    all_refused(&api, "not-a-link-token", &enrollment).await;
    for what in ["check", "enrollment", "accept"] {
        let (status, body) = post(&api, what, json!({})).await;
        assert!(
            status.is_client_error(),
            "{what} without a token: {status} {body}"
        );
    }
    assert_eq!(users(&api).await, 0);
}

/// The code may come with a space or a dash between its groups, as people copy it; a code with a letter in it is a
/// wrong code and counts against the link.
#[sqlx::test]
async fn the_code_may_carry_spaces_or_dashes(db: PgPool) {
    let api = Api::new(db);
    for split in [" ", "-"] {
        let token = link(&api, None).await;
        let enrollment = enroll(&api, &token).await;
        let now = code(&enrollment, 0);
        let email = format!("split{}@example.com", split.trim().len());
        let (status, body) = accept(
            &api,
            &token,
            &email,
            &format!("{}{split}{}", &now[..3], &now[3..]),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{split:?}: {body}");
    }

    let token = link(&api, None).await;
    let enrollment = enroll(&api, &token).await;
    let now = code(&enrollment, 0);
    let (status, _) = accept(&api, &token, EMAIL, &format!("{}a{}", &now[..2], &now[3..])).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(attempts(&api, &token).await, 1);
}

#[sqlx::test]
async fn wrong_codes_count_and_five_end_the_link(db: PgPool) {
    let api = Api::new(db);
    let token = link(&api, None).await;
    let enrollment = enroll(&api, &token).await;
    for n in 1..=invites::MAX_ATTEMPTS {
        let (status, problem) = accept(&api, &token, EMAIL, &wrong(&enrollment)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
        assert_eq!(attempts(&api, &token).await, n);
        let still = post(&api, "check", json!({"token": token})).await.0;
        let expected = if n < invites::MAX_ATTEMPTS {
            StatusCode::OK
        } else {
            StatusCode::NOT_FOUND
        };
        assert_eq!(still, expected, "after {n} wrong codes");
    }
    assert_eq!(
        accept(&api, &token, EMAIL, &code(&enrollment, 0)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(users(&api).await, 0);
}

#[sqlx::test]
async fn a_short_password_or_a_taken_email_leaves_the_link_usable(db: PgPool) {
    let api = Api::new(db);
    let taken = api.household("Taken").await;
    let token = link(&api, None).await;
    let enrollment = enroll(&api, &token).await;
    let short = json!({"token": token, "email": EMAIL, "password": "short", "code": code(&enrollment, 0)});
    assert_eq!(
        post(&api, "accept", short).await.0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let (status, problem) = accept(&api, &token, &taken.email, &code(&enrollment, 0)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(
        attempts(&api, &token).await,
        0,
        "a password or email problem counted as a wrong code"
    );
    assert_eq!(
        accept(&api, &token, EMAIL, &code(&enrollment, 0)).await.0,
        StatusCode::CREATED
    );
}

/// Both acceptances pass the first check before either uses the link: the test holds the link's row until both
/// wait on it, so only the statement that uses the link can stop the second.
#[sqlx::test]
async fn two_acceptances_at_once_make_one_user(db: PgPool) {
    let api = Api::new(db.clone());
    let token = link(&api, None).await;
    let enrollment = enroll(&api, &token).await;
    let now = code(&enrollment, 0);
    let mut hold = db.begin().await.unwrap();
    sqlx::query("select 1 from invites for update")
        .execute(&mut *hold)
        .await
        .unwrap();
    let racers: Vec<_> = ["first@example.com", "second@example.com"]
        .into_iter()
        .map(|email| {
            let (api, token, now) = (Api::new(db.clone()), token.clone(), now.clone());
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
    assert_eq!(
        waiting, 2,
        "the two acceptances never waited on the link together"
    );
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
async fn the_token_is_stored_only_as_its_hash(db: PgPool) {
    let api = Api::new(db);
    let token = link(&api, None).await;
    let row: String = sqlx::query_scalar("select row_to_json(i)::text from invites i")
        .fetch_one(&api.db)
        .await
        .unwrap();
    assert!(!row.contains(&token), "the token is stored as is");
    let hashed: i64 = sqlx::query_scalar("select count(*) from invites where token_hash = $1")
        .bind(auth::token_hash(&token))
        .fetch_one(&api.db)
        .await
        .unwrap();
    assert_eq!(hashed, 1);
}

#[sqlx::test]
async fn a_link_for_a_household_joins_it(db: PgPool) {
    let api = Api::new(db);
    let owner = api.household("Home").await;
    let (_, me) = api.call("GET", "/api/v1/me", Some(&owner.token), None).await;
    let household = Uuid::parse_str(me["household_id"].as_str().unwrap()).unwrap();
    let token = link(&api, Some(household)).await;
    assert_eq!(
        post(&api, "check", json!({"token": token})).await.1["new_household"],
        false
    );
    let enrollment = enroll(&api, &token).await;
    let (_, session) = accept(&api, &token, EMAIL, &code(&enrollment, 0)).await;
    let (_, me) = api
        .call("GET", "/api/v1/me", session["token"].as_str(), None)
        .await;
    assert_eq!(me["household_id"].as_str(), Some(household.to_string().as_str()));
}

#[sqlx::test]
async fn accepting_before_the_authenticator_step_is_refused(db: PgPool) {
    let api = Api::new(db);
    let token = link(&api, None).await;
    let (status, problem) = accept(&api, &token, EMAIL, "123456").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    assert_eq!(
        post(&api, "check", json!({"token": token})).await.0,
        StatusCode::OK
    );
    assert_eq!(attempts(&api, &token).await, 0);
}
