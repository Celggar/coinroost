//! Feedback told from the app: every cap holds when the API is called directly, a full report fits, an image's type
//! comes from its bytes, a draft sent twice is kept once, and a household sees only its own reports.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use coinroost_server::tokens::{self, Scope};
use common::assistant::Mcp;
use common::{Api, User};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower::ServiceExt;

const JPEG: [u8; 4] = [0xFF, 0xD8, 0xFF, 0xE0];
const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// An image of `size` bytes that starts as `kind` does; the rest of it is never read.
fn image(kind: &[u8], size: usize) -> String {
    let mut bytes = kind.to_vec();
    bytes.resize(size, 7);
    STANDARD.encode(bytes)
}

fn report(report_id: &str, message: &str, shots: Value) -> Value {
    let mut body = json!({
        "report_id": report_id, "kind": "problem", "message": message,
        "context": { "path": "/accounts?tab=coverage", "viewport_width": 390, "viewport_height": 844,
                     "color_scheme": "dark", "language": "es", "user_agent": "Mozilla/5.0 (test)" }
    });
    body["shots"] = shots;
    body
}

async fn send(api: &Api, user: &User, body: Value) -> (StatusCode, Value) {
    api.call("POST", "/api/v1/feedback", Some(&user.token), Some(body))
        .await
}

/// A screenshot's bytes and headers, as the browser gets them.
async fn fetch(api: &Api, user: &User, url: &str) -> (StatusCode, String, String, Vec<u8>) {
    let request = Request::builder()
        .uri(url)
        .header(header::AUTHORIZATION, format!("Bearer {}", user.token))
        .body(Body::empty())
        .unwrap();
    let response = api.app.clone().oneshot(request).await.unwrap();
    let head = |name| {
        response
            .headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_owned())
            .unwrap_or_default()
    };
    let (kind, cache) = (head(header::CONTENT_TYPE), head(header::CACHE_CONTROL));
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, kind, cache, bytes)
}

const DRAFT: &str = "0b5c6f2e-3c1d-4a8e-9f00-1d2e3f4a5b6c";

#[sqlx::test]
async fn a_full_report_is_taken(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Teller").await;
    // Every cap at once, at its edge: ten images of 600,000 bytes (8 MB of base64), notes of 500, a message of 4,000.
    let shots: Vec<Value> = (0..10)
        .map(|i| json!({ "image": image(&JPEG, 600_000), "note": format!("{i}{}", "n".repeat(499)) }))
        .collect();
    let body = report(DRAFT, &"m".repeat(4000), json!(shots));
    let (status, made) = send(&api, &user, body.clone()).await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    let notes: Vec<String> = made["shots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["note"].as_str().unwrap()[..1].to_owned())
        .collect();
    assert_eq!(
        notes,
        ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"],
        "each note stays with its screenshot"
    );
    assert_eq!(made["context"]["path"], "/accounts?tab=coverage");
    assert_eq!(made["author"], user.email.as_str());

    let (status, kind, cache, bytes) = fetch(&api, &user, made["shots"][9]["url"].as_str().unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (kind.as_str(), bytes.len(), &bytes[..4]),
        ("image/jpeg", 600_000, &JPEG[..])
    );
    assert!(cache.starts_with("private"), "{cache}");

    // The answer was lost and the panel sends the same draft again: the same report, nothing more.
    let (status, again) = send(&api, &user, body).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["id"], made["id"]);
    let (_, list) = api.call("GET", "/api/v1/feedback", Some(&user.token), None).await;
    assert_eq!(list["items"].as_array().unwrap().len(), 1, "{list}");
}

#[sqlx::test]
async fn every_cap_is_checked_by_the_api(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Checker").await;
    let shot = |image: String, note: &str| json!({ "image": image, "note": note });
    let good = || shot(image(&JPEG, 1000), "");
    let mut no_path = report(DRAFT, "x", json!([]));
    no_path["context"]["path"] = json!("accounts");
    let cases = [
        (report(DRAFT, "", json!([])), "empty_feedback", "#/message"),
        (
            report(DRAFT, &"m".repeat(4001), json!([])),
            "text_length",
            "#/message",
        ),
        (
            report(DRAFT, "x", json!(vec![good(); 11])),
            "too_many_shots",
            "#/shots",
        ),
        (
            report(DRAFT, "x", json!([good(), shot(image(&JPEG, 600_001), "")])),
            "image_too_large",
            "#/shots/1/image",
        ),
        (
            report(DRAFT, "x", json!([shot(image(b"GIF89a", 1000), "")])),
            "image_type",
            "#/shots/0/image",
        ),
        (
            report(DRAFT, "x", json!([shot("not base64!".to_owned(), "")])),
            "image_encoding",
            "#/shots/0/image",
        ),
        (
            report(DRAFT, "x", json!([shot(image(&JPEG, 1000), &"n".repeat(501))])),
            "text_length",
            "#/shots/0/note",
        ),
        (no_path, "path", "#/context/path"),
    ];
    for (body, code, pointer) in cases {
        let (status, problem) = send(&api, &user, body).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{code}: {problem}");
        assert_eq!(
            (problem["code"].as_str(), problem["pointer"].as_str()),
            (Some(code), Some(pointer))
        );
    }
    let mut bug = report(DRAFT, "x", json!([]));
    bug["kind"] = json!("bug");
    assert_eq!(send(&api, &user, bug).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (_, list) = api.call("GET", "/api/v1/feedback", Some(&user.token), None).await;
    assert_eq!(list["items"], json!([]), "nothing refused was kept");
}

#[sqlx::test]
async fn an_image_is_the_type_its_bytes_say(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Painter").await;
    let shots = json!([{ "image": image(&PNG, 2000), "note": "png" }, { "image": image(&JPEG, 2000) }]);
    let (status, made) = send(&api, &user, report(DRAFT, "", shots)).await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    for (i, kind) in [(0, "image/png"), (1, "image/jpeg")] {
        let (status, served, _, _) = fetch(&api, &user, made["shots"][i]["url"].as_str().unwrap()).await;
        assert_eq!((status, served.as_str()), (StatusCode::OK, kind));
        assert_eq!(made["shots"][i]["media_type"], kind);
    }
}

#[sqlx::test]
async fn a_household_sees_and_resolves_only_its_own(db: PgPool) {
    let api = Api::new(db);
    let (user, stranger) = (api.household("Owner").await, api.household("Stranger").await);
    let (_, made) = send(
        &api,
        &user,
        report(DRAFT, "", json!([{ "image": image(&JPEG, 100) }])),
    )
    .await;
    let (id, url) = (
        made["id"].as_str().unwrap(),
        made["shots"][0]["url"].as_str().unwrap(),
    );
    let one = format!("/api/v1/feedback/{id}");

    let (_, theirs) = api
        .call("GET", "/api/v1/feedback", Some(&stranger.token), None)
        .await;
    assert_eq!(theirs["items"], json!([]));
    assert_eq!(
        api.call("GET", &one, Some(&stranger.token), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(fetch(&api, &stranger, url).await.0, StatusCode::NOT_FOUND);
    let resolve = |resolved: bool| Some(json!({ "resolved": resolved }));
    assert_eq!(
        api.call("PATCH", &one, Some(&stranger.token), resolve(true))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    // The same draft id from another household is its own report.
    assert_eq!(
        send(&api, &stranger, report(DRAFT, "mine", json!([]))).await.0,
        StatusCode::CREATED
    );

    let listed = |state: &'static str| {
        let api = &api;
        let token = user.token.clone();
        async move {
            let (_, list) = api
                .call(
                    "GET",
                    &format!("/api/v1/feedback?state={state}"),
                    Some(&token),
                    None,
                )
                .await;
            list["items"].as_array().unwrap().len()
        }
    };
    assert_eq!((listed("open").await, listed("resolved").await), (1, 0));
    let (status, done) = api.call("PATCH", &one, Some(&user.token), resolve(true)).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert!(done["resolved_at"].is_string(), "{done}");
    assert_eq!((listed("open").await, listed("resolved").await), (0, 1));
    let (_, open) = api.call("PATCH", &one, Some(&user.token), resolve(false)).await;
    assert!(open["resolved_at"].is_null(), "{open}");
    assert_eq!(listed("open").await, 1);
}

#[sqlx::test]
async fn the_assistant_sees_each_note_next_to_its_image(db: PgPool) {
    let api = Api::new(db.clone());
    let user = api.household("Teller").await;
    let shots = json!([{ "image": image(&PNG, 300), "note": "the total is wrong" },
                       { "image": image(&JPEG, 400), "note": "here too" }]);
    let (_, made) = send(&api, &user, report(DRAFT, "two screens", shots)).await;
    let token = tokens::add(&db, &user.email, "assistant", Scope::Write)
        .await
        .unwrap();
    let mcp = Mcp::new(&db);

    let listed = mcp.ok(&token, "feedback_list", json!({})).await;
    assert_eq!(
        listed["reports"][0]["screenshot_notes"],
        json!(["the total is wrong", "here too"])
    );
    let call = json!({ "name": "feedback_get", "arguments": { "id": made["id"] } });
    let result = mcp.rpc(&token, "tools/call", call).await;
    let content = result["content"].as_array().unwrap();
    let kinds: Vec<&str> = content.iter().map(|c| c["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["text", "text", "image", "text", "image"], "{result}");
    assert_eq!(content[1]["text"], "Screenshot 1: the total is wrong");
    assert_eq!(content[3]["text"], "Screenshot 2: here too");
    let decoded = |c: &Value| STANDARD.decode(c["data"].as_str().unwrap()).unwrap().len();
    assert_eq!(
        (content[2]["mimeType"].as_str(), decoded(&content[2])),
        (Some("image/png"), 300)
    );
    assert_eq!(
        (content[4]["mimeType"].as_str(), decoded(&content[4])),
        (Some("image/jpeg"), 400)
    );
    // An id is never a path of the caller's making.
    let refused = mcp
        .refused(&token, "feedback_get", json!({ "id": "../transactions" }))
        .await;
    assert!(refused.contains("not a report id"), "{refused}");

    mcp.ok(&token, "feedback_resolve", json!({ "id": made["id"] }))
        .await;
    assert_eq!(
        mcp.ok(&token, "feedback_list", json!({})).await["reports"],
        json!([])
    );
    let all = mcp.ok(&token, "feedback_list", json!({ "state": "all" })).await;
    assert_eq!(all["reports"].as_array().unwrap().len(), 1);
}
