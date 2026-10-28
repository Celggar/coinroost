//! Detection loaded onto existing accounts (`coinroost-server accounts load`): accounts made without it, as a
//! Firefly migration makes them, stop coming back ambiguous when a file is dropped.

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use coinroost_normalizer::detect::AccountRules;
use coinroost_server::account_rules::{self, LoadReport};
use common::{Api, User};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

const BAC_FILE: &[u8] = include_bytes!("../../../spec/fixtures/bac-account/input.csv");
const CARD_FILE: &[u8] = include_bytes!("../../../spec/fixtures/bac-card/input.csv");
const COOPEDUC_FILE: &[u8] = include_bytes!("../../../spec/fixtures/coopeduc/input.pdf");

/// Each pair shares a format; one column of each pair's detection tells them apart (the card fixture is `37…`).
const RULES: &str = r#"
[[account]]
namespace = "bac"
profile = "bac-account"
markers = ["Jane Doe Example"]
[[account]]
namespace = "bac_twin"
profile = "bac-account"
markers = ["no such text"]
[[account]]
namespace = "amex"
profile = "bac-card"
card_prefixes = ["34", "37"]
[[account]]
namespace = "visa"
profile = "bac-card"
card_prefixes = ["4"]
[[account]]
namespace = "coop"
profile = "coopeduc"
label = "AH CORRIENTE"
[[account]]
namespace = "coop_twin"
profile = "coopeduc"
label = "AH NAVIDAD"
"#;

/// An account as a migration leaves it: a profile, no detection.
async fn bare(api: &Api, user: &User, namespace: &str, profile: &str) -> String {
    let body = json!({ "name": namespace, "kind": "checking", "currency": "USD", "namespace": namespace,
                       "profile": profile });
    let (status, account) = api
        .call("POST", "/api/v1/accounts", Some(&user.token), Some(body))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{account}");
    account["id"].as_str().unwrap().to_owned()
}

/// The account a dropped file is matched to, or the detection result when there is none.
async fn detected(api: &Api, user: &User, file: &[u8]) -> Value {
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/imports/preview?file_name=statement.csv")
        .header(header::AUTHORIZATION, format!("Bearer {}", user.token))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(file.to_vec()))
        .unwrap();
    let (status, preview) = api.send(request).await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    if preview["detection"] == "found" {
        preview["account_id"].clone()
    } else {
        preview["detection"].clone()
    }
}

async fn household_of(api: &Api, user: &User) -> Uuid {
    sqlx::query_scalar("select household_id from users where email = $1")
        .bind(&user.email)
        .fetch_one(&api.db)
        .await
        .unwrap()
}

fn rules(text: &str) -> AccountRules {
    AccountRules::from_toml(text).unwrap()
}

#[sqlx::test]
async fn loaded_detection_tells_migrated_accounts_apart(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Migrated").await;
    let household = household_of(&api, &user).await;
    let mut ids = Vec::new();
    for (namespace, profile) in [
        ("bac", "bac-account"),
        ("bac_twin", "bac-account"),
        ("amex", "bac-card"),
        ("visa", "bac-card"),
        ("coop", "coopeduc"),
        ("coop_twin", "coopeduc"),
    ] {
        ids.push(bare(&api, &user, namespace, profile).await);
    }
    for file in [BAC_FILE, CARD_FILE, COOPEDUC_FILE] {
        assert_eq!(detected(&api, &user, file).await, "ambiguous");
    }

    let report = account_rules::load(&api.db, household, &rules(RULES))
        .await
        .unwrap();
    assert_eq!(
        report,
        LoadReport {
            changed: 6,
            unchanged: 0
        }
    );
    assert_eq!(detected(&api, &user, BAC_FILE).await, json!(ids[0]));
    assert_eq!(detected(&api, &user, CARD_FILE).await, json!(ids[2]));
    assert_eq!(detected(&api, &user, COOPEDUC_FILE).await, json!(ids[4]));

    let again = account_rules::load(&api.db, household, &rules(RULES))
        .await
        .unwrap();
    assert_eq!(
        again,
        LoadReport {
            changed: 0,
            unchanged: 6
        }
    );
}

#[sqlx::test]
async fn a_bad_file_changes_no_account(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Untouched").await;
    let household = household_of(&api, &user).await;
    bare(&api, &user, "bac", "bac-account").await;
    bare(&api, &user, "bac_twin", "bac-account").await;
    let good =
        "[[account]]\nnamespace = \"bac\"\nprofile = \"bac-account\"\nmarkers = [\"Jane Doe Example\"]\n";
    let cases = [
        (
            "[[account]]\nnamespace = \"nope\"\nprofile = \"bac-account\"\n",
            "no account with this namespace",
        ),
        (
            "[[account]]\nnamespace = \"bac_twin\"\nprofile = \"bac-account\"\ncard_prefixes = [\"x\"]\n",
            "detection.card_prefixes are 1 to 6 digits each",
        ),
        (
            "[[account]]\nnamespace = \"bac_twin\"\nprofile = \"bac-card\"\n",
            "it reads with profile bac-account",
        ),
        (good, "listed twice"),
    ];
    for (bad, why) in cases {
        let error = account_rules::load(&api.db, household, &rules(&format!("{good}{bad}")))
            .await
            .unwrap_err();
        assert!(error.contains(why), "{error}");
    }
    // The valid rule came first in every file: none of it may have been written.
    let (_, accounts) = api.call("GET", "/api/v1/accounts", Some(&user.token), None).await;
    let detections: Vec<&Value> = accounts["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| &a["detection"])
        .collect();
    let empty = json!({ "markers": [], "card_prefixes": [], "label": null, "number_hash": null });
    assert_eq!(detections, [&empty, &empty]);
}
