//! `firefly fetch` against a test Firefly: GET requests only, every page and every group (even those no list
//! returns), each own account's balance at every month end, and a folder only its owner can read.

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use axum::http::Method;
use coinroost_server::firefly::fetch;
use coinroost_server::firefly::snapshot::Snapshot;
use common::firefly::{TOKEN, TestFirefly, scratch, token_file};
use serde_json::Value;

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[tokio::test]
async fn fetch_reads_everything_with_only_get() {
    let firefly = TestFirefly::start().await;
    let out = scratch().join("snapshot");
    let summary = fetch::fetch(&firefly.url, TOKEN, &out).await.unwrap();
    assert!(summary.contains("Firefly 6.6.3"), "{summary}");

    let requests = firefly.requests();
    let writes: Vec<_> = requests
        .iter()
        .filter(|(method, _)| method != Method::GET)
        .collect();
    assert!(writes.is_empty(), "{writes:?}");
    let asked = |prefix: &str| requests.iter().any(|(_, target)| target.starts_with(prefix));
    assert!(
        asked("/api/v1/transactions?type=all&limit=100&page=6"),
        "every page, 4 a page whatever was asked"
    );
    assert!(asked("/api/v1/accounts?type=all"));

    // Group 2 is in no list and came through its id; 10 is deleted.
    assert!(out.join("groups/2.json").exists());
    assert!(!out.join("groups/10.json").exists());
    let manifest: Value = serde_json::from_slice(&std::fs::read(out.join("manifest.json")).unwrap()).unwrap();
    let found = (
        &manifest["groups_outside_lists"],
        &manifest["groups_deleted"],
        &manifest["month_end_balances"],
    );
    assert_eq!(
        found,
        (&Value::from(1), &Value::from(1), &Value::from(15)),
        "5 own accounts × 3 month ends"
    );

    let snapshot = Snapshot::load(&out).unwrap();
    assert_eq!(
        (snapshot.counts["transactions"], snapshot.journals.len()),
        (24, 25)
    );
    assert_eq!(
        (
            snapshot.counts["accounts"],
            snapshot.counts["tags"],
            snapshot.counts["webhooks"]
        ),
        (14, 3, 0)
    );
    // Webhooks are switched off in this Firefly: recorded as such, not as an empty list.
    assert_eq!(manifest["unavailable"], serde_json::json!(["webhooks"]));
    assert_eq!(snapshot.unavailable, ["webhooks"]);
    assert_eq!(snapshot.version, "6.6.3");

    // The folder holds real data when it is real: the owner alone reads it.
    assert_eq!(
        (
            mode(&out),
            mode(&out.join("pages/accounts")),
            mode(&out.join("manifest.json"))
        ),
        (0o700, 0o700, 0o600)
    );
}

#[tokio::test]
async fn fetch_refuses_what_it_should_not_trust() {
    let firefly = TestFirefly::start().await;
    let dir = scratch();
    let shared = fetch::token(&token_file(&dir, TOKEN, 0o644)).unwrap_err();
    assert!(shared.contains("chmod 600"), "{shared}");

    let refused = fetch::fetch(&firefly.url, "wrong", &dir.join("a"))
        .await
        .unwrap_err();
    assert!(refused.contains("refused the token"), "{refused}");
    let again = fetch::fetch(&firefly.url, TOKEN, &dir).await.unwrap_err();
    assert!(again.contains("already exists"), "{again}");
    let secure = fetch::fetch("https://firefly.example", TOKEN, &dir.join("b"))
        .await
        .unwrap_err();
    assert!(secure.contains("http://"), "{secure}");
}
