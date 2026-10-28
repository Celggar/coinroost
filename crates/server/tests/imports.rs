//! Importing bank files: the server reads them with the account's profile, and the same file twice adds nothing.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;

const BAC_FILE: &[u8] = include_bytes!("../../../spec/fixtures/bac-account/input.csv");
const BAC_EXPECTED: &str = include_str!("../../../spec/fixtures/bac-account/expected.json");
const COOPEDUC_FILE: &[u8] = include_bytes!("../../../spec/fixtures/coopeduc/input.pdf");

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

async fn account(api: &Api, user: &User, name: &str, profile: &str, markers: &[&str]) -> String {
    let body = json!({ "name": name, "kind": "checking", "currency": "USD", "namespace": name.to_lowercase(),
                       "profile": profile, "detection": { "markers": markers } });
    let (status, account) = api
        .call("POST", "/api/v1/accounts", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    account["id"].as_str().unwrap().to_owned()
}

async fn post_file(api: &Api, user: &User, uri: &str, file: &[u8]) -> (StatusCode, Value) {
    let request = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {}", user.token))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(file.to_vec()))
        .unwrap();
    api.send(request).await
}

async fn upload(api: &Api, user: &User, account: &str, file: &[u8]) -> (StatusCode, Value) {
    let uri = format!("/api/v1/accounts/{account}/imports?file_name=statement.csv");
    post_file(api, user, &uri, file).await
}

async fn preview(api: &Api, user: &User, account: Option<&str>, file: &[u8]) -> (StatusCode, Value) {
    let uri = match account {
        Some(id) => format!("/api/v1/imports/preview?file_name=statement.csv&account_id={id}"),
        None => "/api/v1/imports/preview?file_name=statement.csv".to_owned(),
    };
    post_file(api, user, &uri, file).await
}

async fn count_rows(api: &Api, user: &User) -> (usize, usize) {
    let (_, batches) = api.call("GET", "/api/v1/imports", Some(&user.token), None).await;
    let (_, transactions) = api
        .call("GET", "/api/v1/transactions", Some(&user.token), None)
        .await;
    (
        batches["items"].as_array().unwrap().len(),
        transactions["items"].as_array().unwrap().len(),
    )
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

#[sqlx::test]
async fn previews_without_writing(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Preview").await;
    let account = bac_account(&api, &user).await;
    let (status, shown) = preview(&api, &user, Some(&account), BAC_FILE).await;
    assert_eq!(status, StatusCode::OK, "{shown}");
    assert_eq!(shown["detection"], "chosen");
    assert_eq!(shown["reading"]["rows"].as_array().unwrap().len(), 9);
    assert_eq!(count_rows(&api, &user).await, (0, 0));

    // A file the profile cannot read is refused the same way the import refuses it.
    let (status, _) = preview(&api, &user, Some(&account), b"just, some\nwords").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(count_rows(&api, &user).await, (0, 0));
}

#[sqlx::test]
async fn preview_counts_match_the_import(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Counts").await;
    let account = bac_account(&api, &user).await;
    let expected: Value = serde_json::from_str(BAC_EXPECTED).unwrap();

    let (_, before) = preview(&api, &user, Some(&account), BAC_FILE).await;
    let reading = &before["reading"];
    let mut ids: Vec<&str> = reading["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["external_id"].as_str().unwrap())
        .collect();
    let mut reference: Vec<&str> = expected["result"]["transactions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["external_id"].as_str().unwrap())
        .collect();
    ids.sort_unstable();
    reference.sort_unstable();
    assert_eq!(ids, reference);

    let (_, report) = upload(&api, &user, &account, BAC_FILE).await;
    let batch = &report["batch"];
    assert_eq!(batch["added"], reading["new"]);
    assert_eq!(batch["duplicates"], reading["duplicates"]);
    assert_eq!(batch["errors"], reading["errors"]);
    assert_eq!(batch["rows_read"], reading["rows_read"]);
    assert_eq!(
        (&batch["first_date"], &batch["last_date"]),
        (&json!("2026-05-02"), &json!("2026-05-31"))
    );
    assert_eq!(
        (&reading["first_date"], &reading["last_date"]),
        (&batch["first_date"], &batch["last_date"])
    );

    // After the import, the same file previews as nothing new.
    let (_, after) = preview(&api, &user, Some(&account), BAC_FILE).await;
    assert_eq!(after["reading"]["new"], 0);
    assert_eq!(after["reading"]["duplicates"], reading["new"]);
    assert!(
        after["reading"]["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["duplicate"] == true)
    );
}

/// Files dropped without naming the account are matched to one by their header and the account's detection.
#[sqlx::test]
async fn detects_the_account_of_each_file(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Detection").await;
    let (_, none) = preview(&api, &user, None, BAC_FILE).await;
    assert_eq!(
        (&none["detection"], &none["reading"]),
        (&json!("not_found"), &Value::Null)
    );

    let bac = account(&api, &user, "Bac", "bac-account", &[]).await;
    let coop = account(&api, &user, "Coop", "coopeduc", &[]).await;
    // Same format, but its marker is not in the file.
    account(&api, &user, "Other", "bac-account", &["no such text"]).await;
    for (file, id) in [(BAC_FILE, &bac), (COOPEDUC_FILE, &coop)] {
        let (status, found) = preview(&api, &user, None, file).await;
        assert_eq!(status, StatusCode::OK, "{found}");
        assert_eq!(
            (&found["detection"], &found["account_id"]),
            (&json!("found"), &json!(id))
        );
        assert!(found["reading"]["rows"].as_array().is_some_and(|r| !r.is_empty()));
    }

    // Two accounts that both fit: the person chooses.
    let twin = account(&api, &user, "Twin", "bac-account", &[]).await;
    let (_, ambiguous) = preview(&api, &user, None, BAC_FILE).await;
    assert_eq!(ambiguous["detection"], "ambiguous");
    let mut candidates: Vec<&str> = ambiguous["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c.as_str().unwrap())
        .collect();
    candidates.sort_unstable();
    let mut both = [bac.as_str(), twin.as_str()];
    both.sort_unstable();
    assert_eq!(candidates, both);
    assert_eq!(ambiguous["reading"], Value::Null);
}

#[sqlx::test]
async fn coverage_lists_every_account(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Coverage").await;
    let account = bac_account(&api, &user).await;
    account_without_data(&api, &user).await;
    upload(&api, &user, &account, BAC_FILE).await;
    let (status, coverage) = api
        .call(
            "GET",
            "/api/v1/coverage?today=2026-06-20",
            Some(&user.token),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{coverage}");
    let items = coverage["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    let bac = &items[0];
    assert_eq!(bac["account_id"], account.as_str());
    assert_eq!(
        (&bac["latest"], &bac["days_since"], &bac["delay"]),
        (&json!("2026-05-31"), &json!(20), &json!("warning"))
    );
    assert_eq!(
        bac["ranges"],
        json!([{ "from": "2026-05-02", "to": "2026-05-31" }])
    );
    assert_eq!(bac["gaps"], json!([]));
    assert_eq!(
        (&items[1]["delay"], &items[1]["latest"]),
        (&json!("no_data"), &Value::Null)
    );
}

async fn account_without_data(api: &Api, user: &User) {
    let body = json!({ "name": "Cash", "kind": "cash", "currency": "USD", "namespace": "cash" });
    let (status, _) = api
        .call("POST", "/api/v1/accounts", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED);
}
