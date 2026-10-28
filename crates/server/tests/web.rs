//! The web app served next to the API.

use std::fs;
use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use coinroost_server::{AppState, app};
use http_body_util::BodyExt;
use sqlx::PgPool;
use tower::ServiceExt;

async fn get(app: &Router, uri: &str) -> (StatusCode, HeaderMap, String) {
    let request = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    let bytes = body.collect().await.unwrap().to_bytes();
    (
        parts.status,
        parts.headers,
        String::from_utf8_lossy(&bytes).into_owned(),
    )
}

fn header<'a>(headers: &'a HeaderMap, name: &header::HeaderName) -> &'a str {
    headers.get(name).map_or("", |v| v.to_str().unwrap())
}

#[sqlx::test]
async fn serves_the_web_app_and_keeps_api_problems(db: PgPool) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("web-app");
    fs::create_dir_all(dir.join("_app/immutable")).unwrap();
    fs::write(dir.join("index.html"), "<!doctype html><title>app shell</title>").unwrap();
    fs::write(dir.join("_app/immutable/start.1a2b.js"), "export {};").unwrap();
    fs::write(dir.join("robots.txt"), "User-agent: *\nDisallow: /\n").unwrap();
    let app = app(AppState { db }, Some(&dir));

    // The app's own routes load the shell, which is revalidated on every visit.
    for page in ["/", "/import", "/coverage/deep"] {
        let (status, headers, body) = get(&app, page).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        assert!(body.contains("app shell"), "{page}: {body}");
        assert_eq!(header(&headers, &header::CACHE_CONTROL), "no-cache", "{page}");
        assert_eq!(header(&headers, &header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        assert_eq!(header(&headers, &header::X_FRAME_OPTIONS), "DENY");
    }
    let (status, headers, body) = get(&app, "/_app/immutable/start.1a2b.js").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, "export {};"));
    assert!(header(&headers, &header::CONTENT_TYPE).contains("javascript"));
    assert!(header(&headers, &header::CACHE_CONTROL).contains("immutable"));
    let (status, _, _) = get(&app, "/_app/immutable/missing.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, body) = get(&app, "/robots.txt").await;
    assert_eq!((status, body.starts_with("User-agent")), (StatusCode::OK, true));

    // The API keeps its answers: JSON, and problem+json for a path it does not have, never the app shell.
    let (status, _, body) = get(&app, "/api/v1/health").await;
    assert_eq!((status, body.as_str()), (StatusCode::OK, r#"{"status":"ok"}"#));
    for missing in ["/api/v1/nothing-here", "/api/v2/accounts"] {
        let (status, headers, body) = get(&app, missing).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{missing}");
        assert_eq!(
            header(&headers, &header::CONTENT_TYPE),
            "application/problem+json",
            "{missing}"
        );
        assert!(!body.contains("app shell"));
    }
}
