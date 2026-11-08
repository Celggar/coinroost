//! The transaction list by category and by payee, and `sums`: what the matching transactions add up to per
//! currency, over every page and not only the one returned, with money moved between the household's own accounts
//! left out, as `/api/v1/summary` leaves it out of spending and income.

mod common;

use common::Api;
use common::assistant::{Book, book};
use common::sorting::{id, post};
use serde_json::{Value, json};
use sqlx::PgPool;

async fn list(api: &Api, book: &Book, query: &str) -> Value {
    book.rest(api, &format!("/api/v1/transactions?{query}")).await
}

fn descriptions(page: &Value) -> Vec<&str> {
    let items = page["items"].as_array().unwrap();
    items.iter().map(|t| t["description"].as_str().unwrap()).collect()
}

fn sums(pairs: &[(&str, i64)]) -> Value {
    let entries: Vec<Value> = pairs
        .iter()
        .map(|(currency, minor)| json!({ "currency": currency, "amount_minor": minor }))
        .collect();
    json!(entries)
}

/// April's income minus spending per currency, as the summary counts them.
async fn summary_net(api: &Api, book: &Book) -> Value {
    let summary = book
        .rest(api, "/api/v1/summary?from=2026-04-01&to=2026-04-30")
        .await;
    let totals = summary["totals"].as_array().unwrap();
    let net: Vec<Value> = totals
        .iter()
        .map(|t| {
            let minor = t["income"].as_i64().unwrap() - t["spending"].as_i64().unwrap();
            json!({ "currency": t["currency"], "amount_minor": minor })
        })
        .collect();
    json!(net)
}

#[sqlx::test]
async fn category_and_payee_filters_keep_only_their_transactions(db: PgPool) {
    let api = Api::new(db);
    let book = book(&api).await;
    // A second groceries row with another payee, so a category and a payee never pick the same rows.
    let corner = post(
        &api,
        &book.user,
        "/api/v1/payees",
        json!({ "name": "Corner Shop" }),
    )
    .await;
    let corner = id(&corner).to_owned();
    let cafe = list(&api, &book, "q=cafe%20central").await;
    let uri = format!(
        "/api/v1/transactions/{}",
        cafe["items"][0]["id"].as_str().unwrap()
    );
    let body = json!({ "category_id": book.groceries, "payee_id": corner });
    let (status, _) = api.call("PATCH", &uri, Some(&book.token), Some(body)).await;
    assert!(status.is_success(), "{status}");

    for (query, expected) in [
        (
            format!("category_id={}", book.groceries),
            vec!["cafe central", "SUPERMERCADO LA COSTA"],
        ),
        (format!("payee_id={}", book.market), vec!["SUPERMERCADO LA COSTA"]),
        (format!("payee_id={corner}"), vec!["cafe central"]),
        (
            format!("category_id={}&payee_id={corner}", book.groceries),
            vec!["cafe central"],
        ),
        (
            format!("category_id={}&account_id={}", book.salary, book.checking),
            vec!["ACME PLANILLA"],
        ),
        (
            format!("category_id={}&account_id={}", book.dining, book.checking),
            vec![],
        ),
    ] {
        let page = list(&api, &book, &query).await;
        assert_eq!(descriptions(&page), expected, "{query}");
        assert_eq!(page["total"], expected.len(), "{query}");
    }
}

#[sqlx::test]
async fn sums_add_up_every_match_on_every_page(db: PgPool) {
    let api = Api::new(db);
    let book = book(&api).await;
    // USD: -45.50 + 800.00 - 12.00 - 9.00; the move to savings is not money in or out. JPY: -1500.
    let april = sums(&[("JPY", -1500), ("USD", 73_350)]);
    assert_eq!(
        april,
        summary_net(&api, &book).await,
        "the same money the summary counts"
    );

    let mut query = "from=2026-04-01&to=2026-04-30&limit=1".to_owned();
    for page_number in 1..=6 {
        let page = list(&api, &book, &query).await;
        assert_eq!(
            page["sums"], april,
            "page {page_number} sums every match, not its own row"
        );
        let Some(cursor) = page["next_cursor"].as_str() else {
            assert_eq!(page_number, 6, "6 transactions in pages of 1");
            break;
        };
        query = format!("from=2026-04-01&to=2026-04-30&limit=1&cursor={cursor}");
    }

    let groceries = list(&api, &book, &format!("category_id={}", book.groceries)).await;
    assert_eq!(groceries["sums"], sums(&[("USD", -4550)]));
    let dining = list(&api, &book, &format!("category_id={}", book.dining)).await;
    assert_eq!(dining["sums"], sums(&[("JPY", -1500)]));
    let cafes = list(&api, &book, "q=cafe&limit=1").await;
    assert_eq!(cafes["sums"], sums(&[("USD", -2100)]));
    let nothing = list(&api, &book, "q=nothing%20like%20it").await;
    assert_eq!((&nothing["total"], &nothing["sums"]), (&json!(0), &json!([])));
}

/// A two-leg journal from another bookkeeping app, as a Firefly import stores it: each leg in its own account's
/// currency, so the legs need not cancel.
async fn exchange(api: &Api, book: &Book) {
    let me = book.rest(api, "/api/v1/me").await;
    let household: uuid::Uuid = me["household_id"].as_str().unwrap().parse().unwrap();
    let (transaction,): (uuid::Uuid,) = sqlx::query_as(
        "insert into transactions (household_id, date, description) values ($1, '2026-04-28', 'BUY YEN')
         returning id",
    )
    .bind(household)
    .fetch_one(&api.db)
    .await
    .unwrap();
    for (account, minor, currency) in [(&book.checking, -5000, "USD"), (&book.yen, 7000, "JPY")] {
        sqlx::query(
            "insert into legs (transaction_id, household_id, account_id, amount_minor, currency, posted_on)
             values ($1, $2, $3, $4, $5, '2026-04-28')",
        )
        .bind(transaction)
        .bind(household)
        .bind(account.parse::<uuid::Uuid>().unwrap())
        .bind(minor)
        .bind(currency)
        .execute(&api.db)
        .await
        .unwrap();
    }
}

#[sqlx::test]
async fn money_moved_between_own_accounts_adds_nothing(db: PgPool) {
    let api = Api::new(db);
    let book = book(&api).await;
    let april = sums(&[("JPY", -1500), ("USD", 73_350)]);
    // The move to savings: -100.00 on checking and +100.00 on savings.
    let checking = list(&api, &book, &format!("account_id={}", book.checking)).await;
    assert_eq!(
        checking["sums"],
        sums(&[("USD", 73_350)]),
        "the move out of checking is not spending"
    );
    let savings = list(&api, &book, &format!("account_id={}", book.savings)).await;
    assert_eq!((&savings["total"], &savings["sums"]), (&json!(1), &json!([])));

    // One leg filed as a transfer, its other half not imported yet; and money changed from dollars to yen.
    let waiting = json!({ "date": "2026-04-27", "description": "TO SAVINGS LATER", "category_id": book.own,
                          "legs": [{ "account_id": book.checking, "amount_minor": -2500 }] });
    post(&api, &book.user, "/api/v1/transactions", waiting).await;
    exchange(&api, &book).await;
    assert_eq!(
        summary_net(&api, &book).await,
        april,
        "the summary counts neither"
    );

    let all = list(&api, &book, "from=2026-04-01&to=2026-04-30").await;
    assert_eq!((&all["total"], &all["sums"]), (&json!(8), &april));
    let transfers = list(&api, &book, "transfers=true").await;
    assert_eq!((&transfers["total"], &transfers["sums"]), (&json!(2), &json!([])));
    let own = list(&api, &book, &format!("category_id={}", book.own)).await;
    assert_eq!((&own["total"], &own["sums"]), (&json!(2), &json!([])));
}
