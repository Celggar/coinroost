//! Reports over a period, per currency, as positive minor units. Like the summary they read one-leg transactions
//! only and leave out transfer categories: money moved between own accounts is neither spent nor earned.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(monthly))
        .routes(routes!(by_category))
        .routes(routes!(by_payee))
}

/// The one-leg transactions of the period that are not transfers, with their leg (`$1` household, `$2` and `$3` the
/// period).
macro_rules! moving {
    () => {
        "with moving as (
             select t.date, t.category_id, t.payee_id, l.amount_minor, l.currency
             from transactions t join legs l on l.transaction_id = t.id
                  left join categories c on c.id = t.category_id
             where t.household_id = $1 and t.date between $2 and $3
               and (select count(*) from legs o where o.transaction_id = t.id) = 1
               and c.kind is distinct from 'transfer')
         "
    };
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Period {
    /// First date included.
    from: Date,
    /// Last date included.
    to: Date,
}

impl Period {
    fn check(&self) -> ApiResult<()> {
        if self.from > self.to {
            return Err(ApiError::Invalid("from is after to".to_owned()));
        }
        Ok(())
    }
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct MonthTotals {
    /// `YYYY-MM`.
    month: String,
    currency: String,
    spending: i64,
    income: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Monthly {
    /// Months with money in or out, oldest first; then by currency.
    months: Vec<MonthTotals>,
}

/// Spending and income per month.
#[utoipa::path(get, path = "/api/v1/reports/monthly", tag = "reports", params(Period),
    responses(
        (status = OK, description = "Per month and currency", body = Monthly),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The period is not valid", body = Problem)))]
async fn monthly(
    State(state): State<AppState>,
    session: Session,
    Query(period): Query<Period>,
) -> ApiResult<Json<Monthly>> {
    period.check()?;
    let months = sqlx::query_as(concat!(
        moving!(),
        "select to_char(date, 'YYYY-MM') as month, currency,
                coalesce(sum(-amount_minor) filter (where amount_minor < 0), 0)::bigint as spending,
                coalesce(sum(amount_minor) filter (where amount_minor > 0), 0)::bigint as income
         from moving group by 1, 2 order by 1, 2"
    ))
    .bind(session.household_id)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Monthly { months }))
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct CategorySpending {
    /// None: transactions without a category.
    category_id: Option<Uuid>,
    currency: String,
    spending: i64,
    /// How many transactions.
    count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ByCategory {
    /// Per currency, most spent first; ties by category id, uncategorized last.
    items: Vec<CategorySpending>,
}

/// Spending per category.
#[utoipa::path(get, path = "/api/v1/reports/categories", tag = "reports", params(Period),
    responses(
        (status = OK, description = "Per category and currency", body = ByCategory),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The period is not valid", body = Problem)))]
async fn by_category(
    State(state): State<AppState>,
    session: Session,
    Query(period): Query<Period>,
) -> ApiResult<Json<ByCategory>> {
    period.check()?;
    let items = sqlx::query_as(concat!(
        moving!(),
        "select category_id, currency, sum(-amount_minor)::bigint as spending, count(*) as count
         from moving where amount_minor < 0
         group by 1, 2 order by currency, spending desc, category_id nulls last"
    ))
    .bind(session.household_id)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(ByCategory { items }))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PayeePeriod {
    /// First date included.
    from: Date,
    /// Last date included.
    to: Date,
    /// At most this many per currency (default 10, at most 100).
    limit: Option<i64>,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct PayeeSpending {
    /// None: transactions without a payee.
    payee_id: Option<Uuid>,
    currency: String,
    spending: i64,
    count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct ByPayee {
    /// Per currency, most spent first; ties by payee id, no payee last.
    items: Vec<PayeeSpending>,
}

/// The payees most spent on.
#[utoipa::path(get, path = "/api/v1/reports/payees", tag = "reports", params(PayeePeriod),
    responses(
        (status = OK, description = "Per payee and currency", body = ByPayee),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The period or the limit is not valid", body = Problem)))]
async fn by_payee(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<PayeePeriod>,
) -> ApiResult<Json<ByPayee>> {
    Period {
        from: query.from,
        to: query.to,
    }
    .check()?;
    let limit = query.limit.unwrap_or(10);
    if !(1..=100).contains(&limit) {
        return Err(ApiError::Invalid("limit must be between 1 and 100".to_owned()));
    }
    let items = sqlx::query_as(concat!(
        moving!(),
        ", ranked as (
             select payee_id, currency, sum(-amount_minor)::bigint as spending, count(*) as count,
                    row_number() over (partition by currency
                                       order by sum(-amount_minor) desc, payee_id nulls last) as rank
             from moving where amount_minor < 0 group by 1, 2)
         select payee_id, currency, spending, count from ranked where rank <= $4
         order by currency, rank"
    ))
    .bind(session.household_id)
    .bind(query.from)
    .bind(query.to)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(ByPayee { items }))
}
