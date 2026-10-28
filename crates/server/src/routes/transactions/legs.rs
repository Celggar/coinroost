//! The legs of a transaction: checking new ones and attaching the stored ones to a list.

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

use super::{Leg, NewLeg, Transaction};
use crate::error::{ApiError, ApiResult};

/// Fills each transaction's legs, ordered by amount.
pub(super) async fn attach_legs(db: &PgPool, household_id: Uuid, items: &mut [Transaction]) -> ApiResult<()> {
    let ids: Vec<Uuid> = items.iter().map(|t| t.id).collect();
    let legs: Vec<Leg> = sqlx::query_as(
        "select id, transaction_id, account_id, amount_minor, currency, posted_on, statement_text, external_id,
                import_batch_id
         from legs where household_id = $1 and transaction_id = any($2) order by amount_minor, id",
    )
    .bind(household_id)
    .bind(&ids)
    .fetch_all(db)
    .await?;
    let mut by_transaction: HashMap<Uuid, Vec<Leg>> = HashMap::new();
    for leg in legs {
        by_transaction.entry(leg.transaction_id).or_default().push(leg);
    }
    for item in items {
        item.legs = by_transaction.remove(&item.id).unwrap_or_default();
    }
    Ok(())
}

/// The currency of each leg's account, after checking the legs make one valid transaction.
pub(super) async fn check_legs(db: &PgPool, household_id: Uuid, legs: &[NewLeg]) -> ApiResult<Vec<String>> {
    if legs.is_empty() || legs.len() > 2 {
        return Err(ApiError::Invalid(
            "a transaction has one leg, or two for a transfer".to_owned(),
        ));
    }
    if legs.iter().any(|l| l.amount_minor == 0) {
        return Err(ApiError::Invalid("a leg moves a non-zero amount".to_owned()));
    }
    if legs.iter().any(|l| l.statement_text.chars().count() > 1000) {
        return Err(ApiError::Invalid(
            "statement_text has at most 1000 characters".to_owned(),
        ));
    }
    let mut currencies = Vec::with_capacity(legs.len());
    for leg in legs {
        let currency: Option<(String,)> =
            sqlx::query_as("select currency from accounts where id = $1 and household_id = $2")
                .bind(leg.account_id)
                .bind(household_id)
                .fetch_optional(db)
                .await?;
        let (currency,) =
            currency.ok_or_else(|| ApiError::Invalid(format!("no account {}", leg.account_id)))?;
        currencies.push(currency);
    }
    if let [a, b] = legs {
        if a.account_id == b.account_id {
            return Err(ApiError::Invalid(
                "a transfer moves money between two different accounts".to_owned(),
            ));
        }
        if currencies[0] != currencies[1] {
            return Err(ApiError::Invalid(
                "a transfer moves money within one currency".to_owned(),
            ));
        }
        if a.amount_minor.checked_add(b.amount_minor) != Some(0) {
            return Err(ApiError::Invalid(
                "the two legs of a transfer sum to zero".to_owned(),
            ));
        }
    }
    Ok(currencies)
}
