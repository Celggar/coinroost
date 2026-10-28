//! Deleting a transaction. The ledger mirrors the bank: a deleted import comes back when its statement is imported
//! again. A transfer is refused, since deleting it would leave one account's money unexplained: split it first.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(delete_transaction))
}

/// Deletes a transaction with one leg.
#[utoipa::path(delete, path = "/api/v1/transactions/{transaction_id}", tag = "transactions",
    params(("transaction_id" = Uuid, Path, description = "The transaction")),
    responses(
        (status = NO_CONTENT, description = "Deleted"),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such transaction", body = Problem),
        (status = CONFLICT, description = "A transfer: split it first", body = Problem)))]
async fn delete_transaction(
    State(state): State<AppState>,
    session: Session,
    Path(transaction_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    let legs: Option<(i64,)> = sqlx::query_as(
        "select (select count(*) from legs l where l.transaction_id = t.id)
         from transactions t where t.id = $1 and t.household_id = $2 for update",
    )
    .bind(transaction_id)
    .bind(session.household_id)
    .fetch_optional(&mut *tx)
    .await?;
    match legs {
        None => return Err(ApiError::NotFound),
        Some((count,)) if count > 1 => {
            return Err(ApiError::Conflict(
                "a transfer between two accounts: split it first, then delete either side".to_owned(),
            ));
        }
        Some(_) => {}
    }
    sqlx::query("delete from transactions where id = $1 and household_id = $2")
        .bind(transaction_id)
        .bind(session.household_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
