//! Importing bank files: the server reads them with the account's profile, and the same file twice adds nothing.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

const BAC_FILE: &[u8] = include_bytes!("../../../spec/fixtures/bac-account/input.csv");
const BAC_EXPECTED: &str = include_str!("../../../spec/fixtures/bac-account/expected.json");

/// An account read with the fixture's profile and namespace, so its external ids are the fixture's.
async fn bac_account(api: &Api, user: &User) -> String {
    let expected: Value = serde_json::from_str(BAC_EXPECTED).unwrap();
    let body = json!({ "name": "BAC", "kind": "checking", "currency": "USD",
                       "namespace": expected["namespace"], "profile": expected["profile"] });
    let (status, account) = api
        .call("POST", "/api/v1/accounts", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    account["id"].as_str().unwrap().to_owned()
}

async fn upload(api: &Api, user: &User, account: &str, file: &[u8]) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(format!(
            "/api/v1/accounts/{account}/imports?file_name=statement.csv"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", user.token))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(file.to_vec()))
        .unwrap();
    api.send(request).await
}

#[sqlx::test]
async fn creates_and_lists_imports(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Imports").await;
    let account = bac_account(&api, &user).await;
    let expected: Value = serde_json::from_str(BAC_EXPECTED).unwrap();
    let transactions = expected["result"]["transactions"].as_array().unwrap();

    let (status, report) = upload(&api, &user, &account, BAC_FILE).await;
    assert_eq!(status, StatusCode::CREATED, "{report}");
    assert_eq!(report["batch"]["added"], transactions.len());
    assert_eq!(report["batch"]["duplicates"], 0);
    let error_rows: Vec<&Value> = report["row_errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["row"])
        .collect();
    let expected_rows: Vec<&Value> = expected["result"]["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| &e["row"])
        .collect();
    assert_eq!(error_rows, expected_rows);

    // Every leg carries the fixture's external id and amount: the server computes them as the reference does.
    let uri = format!("/api/v1/transactions?account_id={account}&limit=1000");
    let (_, list) = api.call("GET", &uri, Some(&user.token), None).await;
    let mut stored: Vec<(String, i64)> = list["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| {
            let leg = &t["legs"][0];
            (
                leg["external_id"].as_str().unwrap().to_owned(),
                leg["amount_minor"].as_i64().unwrap(),
            )
        })
        .collect();
    let mut reference: Vec<(String, i64)> = transactions
        .iter()
        .map(|t| {
            (
                t["external_id"].as_str().unwrap().to_owned(),
                t["amount_minor"].as_i64().unwrap(),
            )
        })
        .collect();
    stored.sort();
    reference.sort();
    assert_eq!(stored, reference);

    let (_, batches) = api.call("GET", "/api/v1/imports", Some(&user.token), None).await;
    assert_eq!(batches["items"][0]["id"], report["batch"]["id"]);
    assert_eq!(batches["items"][0]["file_sha256"].as_str().unwrap().len(), 64);
}

#[sqlx::test]
async fn import_twice_adds_nothing(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Twice").await;
    let account = bac_account(&api, &user).await;
    let (_, first) = upload(&api, &user, &account, BAC_FILE).await;
    let (status, second) = upload(&api, &user, &account, BAC_FILE).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(second["batch"]["added"], 0);
    assert_eq!(second["batch"]["duplicates"], first["batch"]["added"]);
    let uri = format!("/api/v1/transactions?account_id={account}&limit=1000");
    let (_, list) = api.call("GET", &uri, Some(&user.token), None).await;
    assert_eq!(
        list["items"].as_array().unwrap().len(),
        usize::try_from(first["batch"]["added"].as_u64().unwrap()).unwrap()
    );
}

#[sqlx::test]
async fn refuses_files_the_profile_cannot_read(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Unreadable").await;
    let account = bac_account(&api, &user).await;
    for file in [b"".as_slice(), b"just, some\nwords".as_slice()] {
        let (status, problem) = upload(&api, &user, &account, file).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{problem}");
    }
    let (_, batches) = api.call("GET", "/api/v1/imports", Some(&user.token), None).await;
    assert_eq!(batches["items"], json!([]));
}
