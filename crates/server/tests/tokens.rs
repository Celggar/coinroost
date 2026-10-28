//! Long-lived API tokens: what each scope may do, revoking, and where a token is accepted.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use coinroost_server::tokens::{self, Scope};
use common::Api;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn a_write_token_reads_writes_and_is_marked_used(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Writer").await;
    let token = tokens::add(&db, &user.email, "assistant", Scope::Write)
        .await
        .unwrap();
    assert!(token.starts_with("crt_") && token.len() > 40, "{token}");

    let (status, me) = api.call("GET", "/api/v1/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["email"], user.email.as_str());
    let body = json!({ "name": "Groceries", "kind": "expense" });
    let (status, made) = api
        .call("POST", "/api/v1/categories", Some(&token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");

    // Only the hash is kept, and the use is recorded.
    let (stored, used): (Vec<u8>, Option<time::OffsetDateTime>) =
        sqlx::query_as("select token_hash, last_used_at from api_tokens where name = 'assistant'")
            .fetch_one(&db)
            .await
            .unwrap();
    assert_ne!(stored, token.as_bytes());
    assert!(used.is_some());
}

#[sqlx::test]
async fn a_read_token_may_only_read(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Reader").await;
    let token = tokens::add(&db, &user.email, "dashboard", Scope::Read)
        .await
        .unwrap();

    let (status, _) = api.call("GET", "/api/v1/accounts", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    let body = json!({ "name": "Groceries", "kind": "expense" });
    let (status, problem) = api
        .call("POST", "/api/v1/categories", Some(&token), Some(body))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert!(problem["detail"].as_str().unwrap().contains("only read"));
    // Previews are POSTs too: a read token asks for nothing that could change.
    let (status, _) = api
        .call("POST", "/api/v1/rules/preview", Some(&token), None)
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (_, categories) = api
        .call("GET", "/api/v1/categories", Some(&user.token), None)
        .await;
    assert_eq!(
        categories["items"],
        json!([]),
        "the refused write left nothing behind"
    );
}

#[sqlx::test]
async fn revoked_unknown_and_reused_names(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Revoker").await;
    let token = tokens::add(&db, &user.email, "laptop", Scope::Write)
        .await
        .unwrap();
    assert!(
        tokens::add(&db, &user.email, "laptop", Scope::Read)
            .await
            .is_err(),
        "one live token per name"
    );

    assert!(tokens::revoke(&db, &user.email, "laptop").await.unwrap());
    let (status, _) = api.call("GET", "/api/v1/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        !tokens::revoke(&db, &user.email, "laptop").await.unwrap(),
        "already revoked"
    );

    // The name is free again, and the new token is a different one.
    let again = tokens::add(&db, &user.email, "laptop", Scope::Write)
        .await
        .unwrap();
    assert_ne!(again, token);
    let (status, _) = api.call("GET", "/api/v1/me", Some(&again), None).await;
    assert_eq!(status, StatusCode::OK);

    let forged = format!("crt_{}", "A".repeat(43));
    let (status, _) = api.call("GET", "/api/v1/me", Some(&forged), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(
        tokens::add(&db, "nobody@example.com", "x", Scope::Write)
            .await
            .is_err()
    );
}

#[sqlx::test]
async fn a_token_is_never_taken_from_a_cookie(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Cookie").await;
    let token = tokens::add(&db, &user.email, "assistant", Scope::Write)
        .await
        .unwrap();
    let request = Request::builder()
        .uri("/api/v1/me")
        .header(header::COOKIE, format!("coinroost_session={token}"))
        .body(Body::empty())
        .unwrap();
    let (status, _) = api.send(request).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}
