//! Transactions and their legs. One leg is income or spending on one account; two legs are a transfer between
//! two of the household's accounts, in one currency, summing to zero.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};
use crate::validate;

mod legs;
use legs::{attach_legs, check_legs};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_transactions, create_transaction))
        .routes(routes!(update_transaction))
}

const DEFAULT_LIMIT: i64 = 100;

macro_rules! columns {
    ($t:literal) => {
        concat!(
            $t,
            "id, ",
            $t,
            "date, ",
            $t,
            "description, ",
            $t,
            "payee_id, ",
            $t,
            "payee_set_by, ",
            $t,
            "category_id, ",
            $t,
            "category_set_by, ",
            $t,
            "rule_id, ",
            $t,
            "created_at"
        )
    };
}
const MAX_LIMIT: i64 = 1000;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewLeg {
    account_id: Uuid,
    /// Signed minor units in the account's currency: negative is money leaving the account.
    amount_minor: i64,
    /// The bank's date for this leg; the transaction's date when absent.
    posted_on: Option<Date>,
    #[serde(default)]
    statement_text: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewTransaction {
    date: Date,
    description: String,
    payee_id: Option<Uuid>,
    category_id: Option<Uuid>,
    /// One leg, or two for a transfer.
    legs: Vec<NewLeg>,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Leg {
    id: Uuid,
    #[serde(skip)]
    transaction_id: Uuid,
    account_id: Uuid,
    amount_minor: i64,
    currency: String,
    posted_on: Date,
    statement_text: String,
    /// The deduplication key of an imported leg.
    external_id: Option<String>,
    import_batch_id: Option<Uuid>,
}

/// Who set a category or a payee.
#[derive(Clone, Copy, Debug, Serialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum SetBy {
    /// A rule; applying the rules again may change it.
    Rule,
    /// A person; rules never change it.
    Manual,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Transaction {
    id: Uuid,
    date: Date,
    description: String,
    payee_id: Option<Uuid>,
    payee_set_by: Option<SetBy>,
    category_id: Option<Uuid>,
    category_set_by: Option<SetBy>,
    /// The rule that set the category or payee, while it exists.
    rule_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[sqlx(skip)]
    legs: Vec<Leg>,
}

#[derive(Serialize, ToSchema)]
pub struct Transactions {
    items: Vec<Transaction>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Filter {
    /// Only transactions with a leg on this account.
    account_id: Option<Uuid>,
    /// First date included.
    from: Option<Date>,
    /// Last date included.
    to: Option<Date>,
    /// Only transactions without a category.
    #[serde(default)]
    uncategorized: bool,
    /// Only transfers: transactions with two legs.
    #[serde(default)]
    transfers: bool,
    /// Only transactions whose description contains this text, upper or lower case alike.
    q: Option<String>,
    /// At most this many, newest first (default 100, at most 1000).
    limit: Option<i64>,
}

/// The household's transactions, newest first.
#[utoipa::path(get, path = "/api/v1/transactions", tag = "transactions", params(Filter),
    responses(
        (status = OK, description = "The transactions with their legs", body = Transactions),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "A filter is not valid", body = Problem)))]
async fn list_transactions(
    State(state): State<AppState>,
    session: Session,
    Query(filter): Query<Filter>,
) -> ApiResult<Json<Transactions>> {
    let limit = filter.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::Invalid(format!(
            "limit must be between 1 and {MAX_LIMIT}"
        )));
    }
    let text = filter.q.as_deref().map(str::trim);
    if text.is_some_and(|q| q.is_empty() || q.chars().count() > 200) {
        return Err(ApiError::Invalid("q has 1 to 200 characters".to_owned()));
    }
    let mut items: Vec<Transaction> = sqlx::query_as(
        concat!(
        "select ",
        columns!("t."),
        " from transactions t
         where t.household_id = $1
           and ($2::uuid is null or exists (select 1 from legs l where l.transaction_id = t.id and l.account_id = $2))
           and ($3::date is null or t.date >= $3) and ($4::date is null or t.date <= $4)
           and (not $6 or t.category_id is null)
           and (not $7 or (select count(*) from legs l where l.transaction_id = t.id) = 2)
           and ($8::text is null or strpos(lower(t.description), lower($8)) > 0)
         order by t.date desc, t.created_at desc, t.id limit $5"
    ))
    .bind(session.household_id)
    .bind(filter.account_id)
    .bind(filter.from)
    .bind(filter.to)
    .bind(limit)
    .bind(filter.uncategorized)
    .bind(filter.transfers)
    .bind(text)
    .fetch_all(&state.db)
    .await?;
    attach_legs(&state.db, session.household_id, &mut items).await?;
    Ok(Json(Transactions { items }))
}

/// Records a transaction: one leg, or a transfer of two legs on different accounts that sum to zero.
#[utoipa::path(post, path = "/api/v1/transactions", tag = "transactions", request_body = NewTransaction,
    responses(
        (status = CREATED, description = "The new transaction", body = Transaction),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The legs or a reference are not valid", body = Problem)))]
async fn create_transaction(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewTransaction>,
) -> ApiResult<(StatusCode, Json<Transaction>)> {
    let description = validate::text("description", &body.description, 1000)?;
    let currencies = check_legs(&state.db, session.household_id, &body.legs).await?;
    let mut tx = state.db.begin().await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into transactions (household_id, date, description, payee_id, payee_set_by, category_id,
                                   category_set_by)
         values ($1, $2, $3, $4, case when $4::uuid is null then null else 'manual' end, $5,
                 case when $5::uuid is null then null else 'manual' end) returning id",
    )
    .bind(session.household_id)
    .bind(body.date)
    .bind(&description)
    .bind(body.payee_id)
    .bind(body.category_id)
    .fetch_one(&mut *tx)
    .await?;
    for (leg, currency) in body.legs.iter().zip(currencies) {
        sqlx::query(
            "insert into legs (transaction_id, household_id, account_id, amount_minor, currency, posted_on,
                               statement_text)
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(id)
        .bind(session.household_id)
        .bind(leg.account_id)
        .bind(leg.amount_minor)
        .bind(currency)
        .bind(leg.posted_on.unwrap_or(body.date))
        .bind(leg.statement_text.trim())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let mut created: Vec<Transaction> = sqlx::query_as(concat!(
        "select ",
        columns!(""),
        " from transactions where id = $1"
    ))
    .bind(id)
    .fetch_all(&state.db)
    .await?;
    attach_legs(&state.db, session.household_id, &mut created).await?;
    let created = created
        .pop()
        .ok_or_else(|| ApiError::Internal("transaction vanished".to_owned()))?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// The fields to change; the others stay. What a person sets here, rules never change.
#[allow(
    clippy::option_option,
    reason = "absent, null and a value mean three things in a patch"
)]
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TransactionUpdate {
    /// A category; `null` leaves the transaction without one, and rules may sort it again.
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    category_id: Option<Option<Uuid>>,
    /// A payee; `null` leaves the transaction without one, and rules may name it again.
    #[serde(default, deserialize_with = "present")]
    #[schema(value_type = Option<Uuid>)]
    payee_id: Option<Option<Uuid>>,
}

/// A field that is present, even as `null`.
#[allow(
    clippy::option_option,
    reason = "absent, null and a value mean three things in a patch"
)]
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Sets a transaction's category or payee by hand.
#[utoipa::path(patch, path = "/api/v1/transactions/{transaction_id}", tag = "transactions",
    request_body = TransactionUpdate,
    params(("transaction_id" = Uuid, Path, description = "The transaction")),
    responses(
        (status = OK, description = "The transaction as it is now", body = Transaction),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such transaction", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The category or payee does not exist", body = Problem)))]
async fn update_transaction(
    State(state): State<AppState>,
    session: Session,
    Path(transaction_id): Path<Uuid>,
    Json(body): Json<TransactionUpdate>,
) -> ApiResult<Json<Transaction>> {
    let mut tx = state.db.begin().await?;
    let updated = sqlx::query(
        "update transactions set
             category_id = case when $3 then $4 else category_id end,
             category_set_by = case when $3 then case when $4::uuid is null then null else 'manual' end
                                    else category_set_by end,
             payee_id = case when $5 then $6 else payee_id end,
             payee_set_by = case when $5 then case when $6::uuid is null then null else 'manual' end
                                 else payee_set_by end
         where id = $1 and household_id = $2",
    )
    .bind(transaction_id)
    .bind(session.household_id)
    .bind(body.category_id.is_some())
    .bind(body.category_id.flatten())
    .bind(body.payee_id.is_some())
    .bind(body.payee_id.flatten())
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    // The rule is kept only while it still set something.
    sqlx::query(
        "update transactions set rule_id = null
         where id = $1 and category_set_by is distinct from 'rule' and payee_set_by is distinct from 'rule'",
    )
    .bind(transaction_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut items: Vec<Transaction> = sqlx::query_as(concat!(
        "select ",
        columns!(""),
        " from transactions where id = $1"
    ))
    .bind(transaction_id)
    .fetch_all(&state.db)
    .await?;
    attach_legs(&state.db, session.household_id, &mut items).await?;
    items
        .pop()
        .map(Json)
        .ok_or_else(|| ApiError::Internal("transaction vanished".to_owned()))
}
