//! Who may call the API.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use coinroost_server::auth;
use common::{Api, PASSWORD};
use serde_json::json;
use sqlx::PgPool;

/// Every operation in the contract except the public ones answers 401 without a session, with a made-up
/// token, and with a session that has expired.
#[sqlx::test]
async fn every_operation_requires_a_session(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Expired").await;
    sqlx::query("update sessions set expires_at = now() - interval '1 second'")
        .execute(&api.db)
        .await
        .unwrap();
    let contract = coinroost_server::openapi();
    let mut checked = 0;
    for (path, item) in &contract.paths.paths {
        let operations = [
            ("GET", &item.get),
            ("POST", &item.post),
            ("PUT", &item.put),
            ("PATCH", &item.patch),
            ("DELETE", &item.delete),
        ];
        for (method, operation) in operations {
            let Some(operation) = operation else { continue };
            let public = operation
                .security
                .as_ref()
                .is_some_and(|s| s.len() == 1 && serde_json::to_value(&s[0]).unwrap() == json!({}));
            if public {
                continue;
            }
            let uri = path.replace("{account_id}", "00000000-0000-0000-0000-000000000001");
            for token in [None, Some("not-a-session"), Some(user.token.as_str())] {
                let body = if method == "GET" || method == "DELETE" {
                    None
                } else {
                    Some(json!({}))
                };
                let (status, problem) = api.call(method, &uri, token, body).await;
                assert_eq!(
                    status,
                    StatusCode::UNAUTHORIZED,
                    "{method} {path} with {token:?}: {problem}"
                );
                assert_eq!(problem["status"], 401);
            }
            checked += 1;
        }
    }
    assert!(checked >= 2, "only {checked} protected operations found");
}

#[sqlx::test]
async fn login_needs_password_and_current_code(db: PgPool) {
    let api = Api::new(db);
    let household = auth::create_household(&api.db, "Login").await.unwrap();
    let (_, secret) = auth::create_user(&api.db, household, "Jane@Example.com", PASSWORD)
        .await
        .unwrap();
    let code = auth::current_code(&secret).unwrap();
    let wrong_code = format!("{:06}", (code.parse::<u32>().unwrap() + 1) % 1_000_000);
    let attempt = |email: &str, password: &str, code: &str| json!({ "email": email, "password": password, "code": code });
    for body in [
        attempt("jane@example.com", "wrong password here", &code),
        attempt("jane@example.com", PASSWORD, &wrong_code),
        attempt("nobody@example.com", PASSWORD, &code),
    ] {
        let (status, problem) = api.call("POST", "/api/v1/sessions", None, Some(body)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(problem["detail"], "a valid session is required");
    }

    // The right password and code, with the email in any case: a session as a cookie and a token.
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/sessions")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            attempt(" JANE@example.com", PASSWORD, &code).to_string(),
        ))
        .unwrap();
    let response = tower::ServiceExt::oneshot(api.app.clone(), request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(cookie.contains("HttpOnly") && cookie.contains("Secure") && cookie.contains("SameSite=Strict"));
    let session = cookie.split(';').next().unwrap().to_owned();
    let me = Request::builder()
        .uri("/api/v1/me")
        .header(header::COOKIE, &session)
        .body(Body::empty())
        .unwrap();
    let (status, body) = api.send(me).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["email"], "jane@example.com");

    // The same code a second time is refused, even with the right password.
    let (status, _) = api
        .call(
            "POST",
            "/api/v1/sessions",
            None,
            Some(attempt("jane@example.com", PASSWORD, &code)),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Logging out ends the session.
    let token = session.split_once('=').unwrap().1.to_owned();
    let (status, _) = api
        .call("DELETE", "/api/v1/sessions/current", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = api.call("GET", "/api/v1/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Five failures in a row lock the user, and a locked user is refused even with the right password and code.
#[sqlx::test]
async fn repeated_failures_lock_the_user(db: PgPool) {
    let api = Api::new(db);
    let household = auth::create_household(&api.db, "Lock").await.unwrap();
    let (_, secret) = auth::create_user(&api.db, household, "lock@example.com", PASSWORD)
        .await
        .unwrap();
    for _ in 0..5 {
        let body = json!({ "email": "lock@example.com", "password": "not the password", "code": "000000" });
        let (status, _) = api.call("POST", "/api/v1/sessions", None, Some(body)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
    let code = auth::current_code(&secret).unwrap();
    let body = json!({ "email": "lock@example.com", "password": PASSWORD, "code": code });
    let (status, _) = api.call("POST", "/api/v1/sessions", None, Some(body)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
async fn health_answers_without_a_session(db: PgPool) {
    let api = Api::new(db);
    let (status, body) = api.call("GET", "/api/v1/health", None, None).await;
    let expected = json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") });
    assert_eq!((status, body), (StatusCode::OK, expected));
}

/// The console enrollment gives a working login: the password, and codes from the URI it prints.
#[sqlx::test]
async fn user_add_enrolls_a_user(db: PgPool) {
    let api = Api::new(db.clone());
    let enrolled = auth::enroll(&db, " New@Example.com ", PASSWORD, None)
        .await
        .unwrap();
    assert!(enrolled.uri.starts_with("otpauth://totp/"), "{}", enrolled.uri);
    let totp = totp_rs::Totp::from_url(&enrolled.uri).unwrap();
    assert_eq!(totp.secret().to_base32(), enrolled.secret_base32);
    let code = auth::current_code(totp.secret().as_bytes()).unwrap();
    let login = json!({ "email": "new@example.com", "password": PASSWORD, "code": code });
    let (status, body) = api.call("POST", "/api/v1/sessions", None, Some(login)).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");

    // A second user joins the same household; a taken email adds nobody.
    let second = auth::enroll(&db, "second@example.com", PASSWORD, Some(enrolled.household_id))
        .await
        .unwrap();
    assert_eq!(second.household_id, enrolled.household_id);
    let households: (i64,) = sqlx::query_as("select count(*) from households")
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(
        auth::enroll(&db, "new@example.com", PASSWORD, None)
            .await
            .is_err()
    );
    let after: (i64,) = sqlx::query_as("select count(*) from households")
        .fetch_one(&db)
        .await
        .unwrap();
    assert_eq!(after, households);
    assert!(
        auth::enroll(&db, "short@example.com", "too short", None)
            .await
            .is_err()
    );
}
