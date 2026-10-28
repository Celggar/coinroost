//! Helpers shared by the API tests: an app on a fresh database (`#[sqlx::test]` creates one per test from
//! `DATABASE_URL`), households with a logged-in user, and JSON requests.

#![allow(dead_code)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use coinroost_server::{AppState, auth, router};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

pub const PASSWORD: &str = "correct horse battery";

pub struct Api {
    pub app: Router,
    pub db: PgPool,
}

/// A user who can log in, and the token of their session.
pub struct User {
    pub email: String,
    pub secret: Vec<u8>,
    pub token: String,
}

impl Api {
    pub fn new(db: PgPool) -> Api {
        Api {
            app: router(AppState { db: db.clone() }),
            db,
        }
    }

    /// A new household with one user, logged in.
    pub async fn household(&self, name: &str) -> User {
        let household = auth::create_household(&self.db, name).await.unwrap();
        let email = format!("{}@example.com", name.to_lowercase().replace(' ', "-"));
        let (_, secret) = auth::create_user(&self.db, household, &email, PASSWORD)
            .await
            .unwrap();
        let code = auth::current_code(&secret).unwrap();
        let (status, body) = self
            .call(
                "POST",
                "/api/v1/sessions",
                None,
                Some(serde_json::json!({
                    "email": email, "password": PASSWORD, "code": code
                })),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        User {
            email,
            secret,
            token: body["token"].as_str().unwrap().to_owned(),
        }
    }

    /// Sends a JSON request, with a bearer token when given; returns the status and the JSON body (or null).
    pub async fn call(
        &self,
        method: &str,
        uri: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(token) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let request = match body {
            Some(body) => request
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(body.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        self.send(request).await
    }

    pub async fn send(&self, request: Request<Body>) -> (StatusCode, Value) {
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()))
        };
        (status, json)
    }
}
