//! What the transactions a filter matches add up to, per currency, counted the way `/api/v1/summary` counts money
//! in and out. A transfer stores two legs on two of the household's accounts: in one currency they cancel, and across
//! two (as a Firefly import may store them) they are an exchange, not spending; a one-leg transaction in a transfer
//! category is a transfer waiting for its other half. None of them adds anything.

use serde::Serialize;
use sqlx::PgPool;
use utoipa::ToSchema;
use uuid::Uuid;

use super::Filter;
use crate::error::ApiResult;

/// The signed total of one currency.
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct CurrencySum {
    currency: String,
    /// Minor units: below zero more money went out than came in.
    amount_minor: i64,
}

/// The sums of every row [`matching`] a filter, not only a page's: the cursor and the limit play no part.
pub(super) async fn sums(
    db: &PgPool,
    household_id: Uuid,
    filter: &Filter,
    text: Option<&str>,
) -> ApiResult<Vec<CurrencySum>> {
    let sums = bind_filter!(
        sqlx::query_as(concat!(
            "select l.currency, sum(l.amount_minor)::bigint as amount_minor
             from (select t.id, t.category_id",
            matching!(),
            ") t
             join legs l on l.transaction_id = t.id
             left join categories c on c.id = t.category_id
             where c.kind is distinct from 'transfer'
               and (select count(*) from legs o where o.transaction_id = t.id) = 1
             group by l.currency
             order by l.currency"
        )),
        household_id,
        filter,
        text
    )
    .fetch_all(db)
    .await?;
    Ok(sums)
}
