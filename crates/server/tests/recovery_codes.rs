//! Recovery codes: ten single-use codes per user, made at enrollment and on demand; a login with one retires the
//! authenticator, ends the user's other sessions and limits the new session to setting up a new authenticator.

mod common;

use std::time::{SystemTime, UNIX_EPOCH};

use axum::http::StatusCode;
use coinroost_server::{invites, tokens, totp};
use common::{Api, PASSWORD, User};
use serde_json::{Value, json};
use sqlx::PgPool;

/// As if the next 30-second window had come: the code the authenticator shows now may be used once more.
async fn next_window(api: &Api, user: &User) {
    sqlx::query("update users set totp_last_step = 0 where email = $1")
        .bind(&user.email)
        .execute(&api.db)
        .await
        .unwrap();
}

/// A new set for the user, made the way the Security page makes it.
async fn codes(api: &Api, user: &User) -> Vec<String> {
    next_window(api, user).await;
    let body = json!({"code": totp::current_code(&user.secret).unwrap()});
    let (status, set) = api
        .call("POST", "/api/v1/me/recovery-codes", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{set}");
    set["codes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap().to_owned())
        .collect()
}

async fn log_in(api: &Api, email: &str, second: Value) -> (StatusCode, Value) {
    let mut body = json!({"email": email, "password": PASSWORD});
    body.as_object_mut()
        .unwrap()
        .extend(second.as_object().unwrap().clone());
    api.call("POST", "/api/v1/sessions", None, Some(body)).await
}

async fn recover(api: &Api, user: &User, code: &str) -> (StatusCode, Value) {
    log_in(api, &user.email, json!({"recovery_code": code})).await
}

async fn status_of(api: &Api, token: &str, method: &str, path: &str) -> StatusCode {
    let body = matches!(method, "POST" | "PUT" | "PATCH").then(|| json!({}));
    api.call(method, path, Some(token), body).await.0
}

/// A contract path with every `{parameter}` filled with an id that exists nowhere.
fn with_ids(path: &str) -> String {
    path.split('/')
        .map(|part| {
            if part.starts_with('{') {
                "00000000-0000-4000-8000-000000000000"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn totp_now(uri: &str) -> String {
    let totp = totp_rs::Totp::from_url(uri).unwrap();
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    totp.generate(now).to_string()
}

#[sqlx::test]
async fn a_recovery_code_logs_in_once_and_reuse_fails(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Reuse").await;
    let set = codes(&api, &user).await;
    assert_eq!(set.len(), 10);
    let distinct: std::collections::HashSet<_> = set.iter().collect();
    assert_eq!(distinct.len(), 10, "{set:?}");
    assert!(set.iter().all(|c| c.len() == 11 && &c[5..6] == "-"), "{set:?}");

    let (status, body) = recover(&api, &user, &set[0]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let (status, _) = recover(&api, &user, &set[0]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a used code works again");

    // Typed loosely: upper case, a space instead of the dash.
    let loose = set[1].to_uppercase().replace('-', " ");
    let (status, body) = recover(&api, &user, &loose).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let token = body["token"].as_str().unwrap();
    let (_, left) = api
        .call("GET", "/api/v1/me/recovery-codes", Some(token), None)
        .await;
    // The session is limited until a new authenticator is set up, so the count comes from the table.
    assert_eq!(left["status"], 403, "{left}");
    let unused: i64 = sqlx::query_scalar("select count(*) from recovery_codes where used_at is null")
        .fetch_one(&api.db)
        .await
        .unwrap();
    assert_eq!(unused, 8);
}

#[sqlx::test]
async fn a_login_needs_exactly_one_second_step(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Exactly One").await;
    let set = codes(&api, &user).await;
    let code = totp::current_code(&user.secret).unwrap();
    let (status, _) = log_in(&api, &user.email, json!({"code": code, "recovery_code": set[0]})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = log_in(&api, &user.email, json!({})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn wrong_recovery_codes_lock_the_user(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Recovery Lock").await;
    let set = codes(&api, &user).await;
    for _ in 0..5 {
        let (status, _) = recover(&api, &user, "zzzzz-zzzzz").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let (status, _) = recover(&api, &user, &set[0]).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a locked user got in with a recovery code"
    );
    next_window(&api, &user).await;
    let code = totp::current_code(&user.secret).unwrap();
    let (status, _) = log_in(&api, &user.email, json!({"code": code})).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a locked user got in with the authenticator"
    );
}

#[sqlx::test]
async fn a_right_code_with_a_wrong_password_fails_and_counts(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Wrong Password").await;
    let set = codes(&api, &user).await;
    let body = json!({"email": user.email, "password": "not the password at all", "recovery_code": set[0]});
    let (status, _) = api.call("POST", "/api/v1/sessions", None, Some(body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (failed, used): (i32, i64) = sqlx::query_as(
        "select failed_logins, (select count(*) from recovery_codes where used_at is not null) from users",
    )
    .fetch_one(&api.db)
    .await
    .unwrap();
    assert_eq!(
        (failed, used),
        (1, 0),
        "a failed login spent the code or was not counted"
    );
}

#[sqlx::test]
async fn regenerating_needs_a_fresh_code_and_retires_the_old_set(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Regenerate").await;
    // The code that just logged in cannot be replayed to make a set.
    let replay = json!({"code": totp::current_code(&user.secret).unwrap()});
    let (status, body) = api
        .call(
            "POST",
            "/api/v1/me/recovery-codes",
            Some(&user.token),
            Some(replay),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");

    let (_, none) = api
        .call("GET", "/api/v1/me/recovery-codes", Some(&user.token), None)
        .await;
    assert_eq!(
        (none["remaining"].as_i64(), none["created_at"].is_null()),
        (Some(0), true),
        "{none}"
    );
    let old = codes(&api, &user).await;
    let new = codes(&api, &user).await;
    assert!(old.iter().all(|c| !new.contains(c)));
    let (_, status_body) = api
        .call("GET", "/api/v1/me/recovery-codes", Some(&user.token), None)
        .await;
    assert_eq!(status_body["remaining"], 10, "{status_body}");
    assert_eq!(status_body["size"], 10, "{status_body}");

    let (status, _) = recover(&api, &user, &old[0]).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a code of the replaced set still works"
    );
    let (status, body) = recover(&api, &user, &new[0]).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[sqlx::test]
async fn wrong_codes_when_regenerating_lock_the_user(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Regenerate Lock").await;
    for _ in 0..5 {
        let (status, _) = api
            .call(
                "POST",
                "/api/v1/me/recovery-codes",
                Some(&user.token),
                Some(json!({"code": "000000"})),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }
    next_window(&api, &user).await;
    let right = json!({"code": totp::current_code(&user.secret).unwrap()});
    let (status, body) = api
        .call(
            "POST",
            "/api/v1/me/recovery-codes",
            Some(&user.token),
            Some(right.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "a locked user made a set: {body}");
    let (status, _) = log_in(&api, &user.email, right).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the lock does not hold at the login"
    );
}

#[sqlx::test]
async fn a_recovery_login_ends_other_sessions(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Other Sessions").await;
    let set = codes(&api, &user).await;
    assert_eq!(
        status_of(&api, &user.token, "GET", "/api/v1/accounts").await,
        StatusCode::OK
    );
    let (status, _) = recover(&api, &user, &set[0]).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(
        status_of(&api, &user.token, "GET", "/api/v1/me").await,
        StatusCode::UNAUTHORIZED,
        "the lost phone's session lives on"
    );
}

#[sqlx::test]
async fn a_recovering_session_is_limited_to_the_new_authenticator(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Limited").await;
    let set = codes(&api, &user).await;
    let api_token = tokens::add(&api.db, &user.email, "laptop", tokens::Scope::Read)
        .await
        .unwrap();
    let (_, body) = recover(&api, &user, &set[0]).await;
    let token = body["token"].as_str().unwrap().to_owned();

    let allowed = [
        ("GET", "/api/v1/me"),
        ("POST", "/api/v1/me/authenticator"),
        ("POST", "/api/v1/me/authenticator/confirm"),
        ("DELETE", "/api/v1/sessions/current"),
    ];
    let contract: Value = serde_json::from_str(include_str!("../../../spec/openapi.json")).unwrap();
    let mut checked = 0;
    for (path, operations) in contract["paths"].as_object().unwrap() {
        let path = with_ids(path);
        for (method, operation) in operations.as_object().unwrap() {
            let method = method.to_uppercase();
            let public = operation["security"] == json!([{}]);
            if public || allowed.contains(&(method.as_str(), path.as_str())) {
                continue;
            }
            let status = status_of(&api, &token, &method, &path).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{method} {path} is open to a recovering session"
            );
            checked += 1;
        }
    }
    assert!(checked > 25, "only {checked} routes were walked");

    let (status, me) = api.call("GET", "/api/v1/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["reenroll_required"], true, "{me}");
    // API tokens are separate credentials and keep working.
    assert_eq!(
        status_of(&api, &api_token, "GET", "/api/v1/accounts").await,
        StatusCode::OK
    );
    assert_eq!(
        status_of(&api, &token, "DELETE", "/api/v1/sessions/current").await,
        StatusCode::NO_CONTENT
    );
}

#[sqlx::test]
async fn the_old_authenticator_dies_and_a_new_one_takes_over(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("New Authenticator").await;
    let set = codes(&api, &user).await;
    let (_, body) = recover(&api, &user, &set[0]).await;
    let token = body["token"].as_str().unwrap().to_owned();

    next_window(&api, &user).await;
    let old_code = json!({"code": totp::current_code(&user.secret).unwrap()});
    let (status, _) = log_in(&api, &user.email, old_code.clone()).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the lost phone's codes still log in"
    );

    let (status, first) = api
        .call("POST", "/api/v1/me/authenticator", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (_, again) = api
        .call("POST", "/api/v1/me/authenticator", Some(&token), None)
        .await;
    assert_eq!(
        first["secret"], again["secret"],
        "the secret changed between calls"
    );
    let uri = first["uri"].as_str().unwrap();

    let (status, _) = api
        .call(
            "POST",
            "/api/v1/me/authenticator/confirm",
            Some(&token),
            Some(old_code.clone()),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "the old secret confirmed the new one"
    );
    let new_code = json!({"code": totp_now(uri)});
    let (status, body) = api
        .call(
            "POST",
            "/api/v1/me/authenticator/confirm",
            Some(&token),
            Some(new_code),
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");

    let (_, me) = api.call("GET", "/api/v1/me", Some(&token), None).await;
    assert_eq!(me["reenroll_required"], false, "{me}");
    assert_eq!(
        status_of(&api, &token, "GET", "/api/v1/accounts").await,
        StatusCode::OK
    );
    let (status, _) = api
        .call("POST", "/api/v1/me/authenticator", Some(&token), None)
        .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "a working account was offered a new authenticator"
    );

    next_window(&api, &user).await;
    let (status, _) = log_in(&api, &user.email, old_code).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "the old secret came back");
    let (status, body) = log_in(&api, &user.email, json!({"code": totp_now(uri)})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}

#[sqlx::test]
async fn a_second_recovery_replaces_the_secret_being_set_up(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Second Recovery").await;
    let set = codes(&api, &user).await;
    let (_, body) = recover(&api, &user, &set[0]).await;
    let first_token = body["token"].as_str().unwrap().to_owned();
    let (_, abandoned) = api
        .call("POST", "/api/v1/me/authenticator", Some(&first_token), None)
        .await;

    let (status, body) = recover(&api, &user, &set[1]).await;
    assert_eq!(
        status,
        StatusCode::CREATED,
        "an abandoned set-up cannot be resumed"
    );
    let token = body["token"].as_str().unwrap().to_owned();
    let (_, fresh) = api
        .call("POST", "/api/v1/me/authenticator", Some(&token), None)
        .await;
    assert_ne!(
        abandoned["secret"], fresh["secret"],
        "a secret shown to an earlier session survived"
    );
    let stale = json!({"code": totp_now(abandoned["uri"].as_str().unwrap())});
    let (status, _) = api
        .call(
            "POST",
            "/api/v1/me/authenticator/confirm",
            Some(&token),
            Some(stale),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[sqlx::test]
async fn concurrent_logins_with_one_code_make_one_session(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Concurrent").await;
    let set = codes(&api, &user).await;
    // Both logins find the code unused, then wait at the lock; neither can spend it until this is released.
    let mut hold = db.begin().await.unwrap();
    sqlx::query("lock table recovery_codes in exclusive mode")
        .execute(&mut *hold)
        .await
        .unwrap();
    let racers: Vec<_> = (0..2)
        .map(|_| {
            let api = Api::new(db.clone());
            let body = json!({"email": user.email, "password": PASSWORD, "recovery_code": set[0]});
            tokio::spawn(async move { api.call("POST", "/api/v1/sessions", None, Some(body)).await.0 })
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
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert_eq!(waiting, 2, "the two logins never waited together");
    hold.rollback().await.unwrap();
    let mut statuses = Vec::new();
    for racer in racers {
        statuses.push(racer.await.unwrap());
    }
    statuses.sort();
    assert_eq!(statuses, [StatusCode::CREATED, StatusCode::UNAUTHORIZED]);
}

#[sqlx::test]
async fn accepting_a_setup_link_hands_out_working_codes(db: PgPool) {
    let api = Api::new(db);
    let link = invites::create(&api.db, None).await.unwrap();
    let body = json!({"token": link.token, "email": "new@example.com"});
    let (_, enrollment) = api
        .call("POST", "/api/v1/invites/enrollment", None, Some(body))
        .await;
    let code = totp_now(enrollment["uri"].as_str().unwrap());
    let body = json!({"token": link.token, "email": "new@example.com", "password": PASSWORD, "code": code});
    let (status, accepted) = api.call("POST", "/api/v1/invites/accept", None, Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "{accepted}");
    let set = accepted["recovery_codes"].as_array().unwrap();
    assert_eq!(set.len(), 10, "{accepted}");
    let (status, body) = log_in(&api, "new@example.com", json!({"recovery_code": set[3]})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
}
