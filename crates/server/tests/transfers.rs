//! Pairing at import: the two legs of a transfer arrive with two statements, and the second import pairs them
//! when, and only when, the pair is certain.

mod common;

use axum::http::StatusCode;
use common::Api;
use common::sorting::{by_description, transactions};
use common::transfers::*;
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test]
async fn legs_imported_apart_pair_up(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Paired").await;
    transfer_rule(&api, &user, "Own transfers", "TRANSFER").await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    let first = statement(&[
        ("04/05/2026", "TRANSFER TO SAVINGS", -10000),
        ("05/05/2026", "COFFEE", -350),
    ]);
    import(&api, &user, &checking, first).await;
    assert_eq!(
        transfers(&api, &user).await.len(),
        0,
        "the other leg has not arrived"
    );

    let second = statement(&[("05/05/2026", "TRANSFER FROM CHECKING", 10000)]);
    import(&api, &user, &savings, second.clone()).await;
    let made = transfers(&api, &user).await;
    assert_eq!(made.len(), 1);
    let transfer = &made[0];
    assert_eq!(transfer["description"], "TRANSFER TO SAVINGS", "the outflow's");
    let legs = transfer["legs"].as_array().unwrap();
    let mut sides: Vec<_> = legs
        .iter()
        .map(|l| {
            (
                l["account_id"].as_str().unwrap(),
                l["amount_minor"].as_i64().unwrap(),
            )
        })
        .collect();
    sides.sort_unstable();
    let mut want = vec![(checking.as_str(), -10000), (savings.as_str(), 10000)];
    want.sort_unstable();
    assert_eq!(sides, want);
    assert!(legs.iter().all(|l| l["external_id"].is_string()));
    assert_eq!(
        transactions(&api, &user, "").await.len(),
        2,
        "coffee and the transfer"
    );
    let totals = balances(&api, &user).await;
    assert_eq!((totals[&checking], totals[&savings]), (-10350, 10000));

    // Importing the second statement again adds nothing: its leg is in the transfer.
    let again = import(&api, &user, &savings, second).await;
    assert_eq!(
        (
            again["batch"]["added"].as_i64(),
            again["batch"]["duplicates"].as_i64()
        ),
        (Some(0), Some(1))
    );
    assert_eq!(transfers(&api, &user).await.len(), 1);
}

#[sqlx::test]
async fn equal_amount_third_leg_stays_ambiguous(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Unsure").await;
    transfer_rule(&api, &user, "Own transfers", "TRANSFER").await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    let card = account(&api, &user, "Card").await;
    import(
        &api,
        &user,
        &savings,
        statement(&[("06/05/2026", "TRANSFER IN", 5100)]),
    )
    .await;
    import(
        &api,
        &user,
        &card,
        statement(&[("07/05/2026", "TRANSFER IN", 5100)]),
    )
    .await;
    let before = legs(&api, &user).await;
    import(
        &api,
        &user,
        &checking,
        statement(&[("06/05/2026", "TRANSFER OUT", -5100)]),
    )
    .await;

    assert_eq!(
        transfers(&api, &user).await.len(),
        0,
        "two inflows could be its other side"
    );
    let (waiting, _) = preview(&api, &user).await;
    assert_eq!(waiting["pairs"], json!([]));
    let unmatched = waiting["unmatched"].as_array().unwrap();
    assert_eq!(unmatched.len(), 3);
    let outflow = unmatched
        .iter()
        .find(|u| u["leg"]["amount_minor"] == -5100)
        .unwrap();
    assert_eq!(outflow["reason"], "several_candidates");
    assert_eq!(outflow["candidates"].as_array().unwrap().len(), 2);
    let after = legs(&api, &user).await;
    assert!(
        before.iter().all(|(id, leg)| after.get(id) == Some(leg)),
        "nothing touched"
    );
}

#[sqlx::test]
async fn manual_inflow_blocks_auto_pairing(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Chosen").await;
    let own = transfer_rule(&api, &user, "Own transfers", "TRANSFER").await;
    let (_, deposits) = api
        .call(
            "POST",
            "/api/v1/categories",
            Some(&user.token),
            Some(json!({ "name": "Savings deposits", "kind": "transfer" })),
        )
        .await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    import(
        &api,
        &user,
        &savings,
        statement(&[("08/05/2026", "TRANSFER FROM CHECKING", 7000)]),
    )
    .await;
    let items = transactions(&api, &user, "").await;
    let inflow = by_description(&items, "TRANSFER FROM")[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, _) = api
        .call(
            "PATCH",
            &format!("/api/v1/transactions/{inflow}"),
            Some(&user.token),
            Some(json!({ "category_id": deposits["id"] })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);

    import(
        &api,
        &user,
        &checking,
        statement(&[("08/05/2026", "TRANSFER TO SAVINGS", -7000)]),
    )
    .await;
    assert_eq!(
        transfers(&api, &user).await.len(),
        0,
        "pairing would drop a category a person chose"
    );
    let (waiting, tag) = preview(&api, &user).await;
    assert_eq!(waiting["pairs"].as_array().unwrap().len(), 1);
    assert_eq!(waiting["pairs"][0]["drops_manual"], true);

    let (status, applied) = apply(&api, &user, Some(&tag)).await;
    assert_eq!((status, applied["paired"].as_i64()), (StatusCode::OK, Some(1)));
    let made = transfers(&api, &user).await;
    assert_eq!(made[0]["category_id"], own.as_str(), "the outflow's category");
}

#[sqlx::test]
async fn transfer_preview_then_apply_with_the_same_etag(db: PgPool) {
    let api = Api::new(db);
    let user = api.household("Waiting").await;
    let checking = account(&api, &user, "Checking").await;
    let savings = account(&api, &user, "Savings").await;
    // No rule yet: neither leg is a transfer, so the imports pair nothing.
    import(
        &api,
        &user,
        &checking,
        statement(&[("11/05/2026", "MOVE TO SAVINGS", -8000)]),
    )
    .await;
    import(
        &api,
        &user,
        &savings,
        statement(&[("12/05/2026", "MOVE FROM CHECKING", 8000)]),
    )
    .await;
    assert_eq!(preview(&api, &user).await.0["pairs"], json!([]));

    // A rule makes them transfers once the rules are applied; the pair then waits in the preview.
    transfer_rule(&api, &user, "Own transfers", "MOVE").await;
    let (_, rule_tag) = common::sorting::preview(&api, &user).await;
    let (status, _) = common::sorting::apply(&api, &user, Some(&rule_tag)).await;
    assert_eq!(status, StatusCode::OK);
    let (waiting, tag) = preview(&api, &user).await;
    assert_eq!(waiting["pairs"].as_array().unwrap().len(), 1);
    assert_eq!(waiting["pairs"][0]["pass"], 1);
    assert_eq!(transfers(&api, &user).await.len(), 0, "a preview writes nothing");

    assert_eq!(
        apply(&api, &user, None).await.0,
        StatusCode::PRECONDITION_REQUIRED
    );
    assert_eq!(
        apply(&api, &user, Some("\"stale\"")).await.0,
        StatusCode::PRECONDITION_FAILED
    );
    let (status, applied) = apply(&api, &user, Some(&tag)).await;
    assert_eq!((status, applied["paired"].as_i64()), (StatusCode::OK, Some(1)));
    assert_eq!(transfers(&api, &user).await.len(), 1);
    let (after, new_tag) = preview(&api, &user).await;
    assert_eq!(after["pairs"], json!([]));
    assert_ne!(new_tag, tag);
    assert_eq!(
        apply(&api, &user, Some(&tag)).await.0,
        StatusCode::PRECONDITION_FAILED
    );
}
