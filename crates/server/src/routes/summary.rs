//! Money in and out over a period, with transfers between own accounts left out. A paired transfer counts in
//! neither; a transfer leg still waiting for its other half is shown apart, so the totals of a tool that counts it
//! as spending can be rebuilt: there, spent = `spending + unpaired_transfer_out`.

use axum::Json;
use axum::extract::{Query, State};
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(summary))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Period {
    /// First date included.
    from: Date,
    /// Last date included.
    to: Date,
}

/// Totals in one currency, as positive minor units.
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Totals {
    currency: String,
    /// Money out of one-leg transactions whose category is not a transfer (uncategorized included).
    spending: i64,
    /// Money in of one-leg transactions whose category is not a transfer.
    income: i64,
    /// Money out of transfer legs not paired yet.
    unpaired_transfer_out: i64,
    /// Money in of transfer legs not paired yet.
    unpaired_transfer_in: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Summary {
    from: Date,
    to: Date,
    /// One entry per currency with money in the period.
    totals: Vec<Totals>,
}

/// Spending and income over a period, transfers left out.
#[utoipa::path(get, path = "/api/v1/summary", tag = "summary", params(Period),
    responses(
        (status = OK, description = "The totals per currency", body = Summary),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The period is not valid", body = Problem)))]
async fn summary(
    State(state): State<AppState>,
    session: Session,
    Query(period): Query<Period>,
) -> ApiResult<Json<Summary>> {
    if period.from > period.to {
        return Err(ApiError::Invalid("from is after to".to_owned()));
    }
    let totals: Vec<Totals> = sqlx::query_as(
        "select l.currency,
                coalesce(sum(-l.amount_minor) filter (where l.amount_minor < 0 and not transfer), 0)::bigint
                    as spending,
                coalesce(sum(l.amount_minor) filter (where l.amount_minor > 0 and not transfer), 0)::bigint
                    as income,
                coalesce(sum(-l.amount_minor) filter (where l.amount_minor < 0 and transfer), 0)::bigint
                    as unpaired_transfer_out,
                coalesce(sum(l.amount_minor) filter (where l.amount_minor > 0 and transfer), 0)::bigint
                    as unpaired_transfer_in
         from (select t.id, coalesce(c.kind = 'transfer', false) as transfer
               from transactions t left join categories c on c.id = t.category_id
               where t.household_id = $1 and t.date between $2 and $3
                 and (select count(*) from legs o where o.transaction_id = t.id) = 1) t
         join legs l on l.transaction_id = t.id
         group by l.currency
         order by l.currency",
    )
    .bind(session.household_id)
    .bind(period.from)
    .bind(period.to)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Summary {
        from: period.from,
        to: period.to,
        totals,
    }))
}
