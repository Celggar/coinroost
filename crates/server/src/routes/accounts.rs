//! The household's accounts. An account's `namespace` goes into the `external_id` of everything imported into
//! it, and its `profile` says how its bank exports are read.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use coinroost_normalizer::Profile;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};
use crate::validate;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list_accounts, create_account))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Cash,
    Loan,
    Investment,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewAccount {
    name: String,
    kind: AccountKind,
    /// ISO 4217 code.
    currency: String,
    /// Lower case letters, digits, `_` and `-`; cannot change once the account has transactions.
    namespace: String,
    /// Id of the bank profile its exports are read with (`coinroost profiles` lists them).
    profile: Option<String>,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Account {
    id: Uuid,
    name: String,
    kind: AccountKind,
    currency: String,
    namespace: String,
    profile: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
pub struct Accounts {
    items: Vec<Account>,
}

macro_rules! columns {
    () => {
        "id, name, kind, currency, namespace, profile, created_at"
    };
}

/// The household's accounts, by name.
#[utoipa::path(get, path = "/api/v1/accounts", tag = "accounts",
    responses(
        (status = OK, description = "The accounts", body = Accounts),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_accounts(State(state): State<AppState>, session: Session) -> ApiResult<Json<Accounts>> {
    let items = sqlx::query_as(concat!(
        "select ",
        columns!(),
        " from accounts where household_id = $1 order by name"
    ))
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Accounts { items }))
}

/// Opens an account. Its currency must be the profile's, when it has one.
#[utoipa::path(post, path = "/api/v1/accounts", tag = "accounts", request_body = NewAccount,
    responses(
        (status = CREATED, description = "The new account", body = Account),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The name or namespace is taken", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "A value is not valid", body = Problem)))]
async fn create_account(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewAccount>,
) -> ApiResult<(StatusCode, Json<Account>)> {
    let name = validate::text("name", &body.name, 200)?;
    let currency = validate::currency(&body.currency)?;
    let namespace = validate::namespace(&body.namespace)?;
    if let Some(id) = &body.profile {
        let profile = Profile::find(id).ok_or_else(|| ApiError::Invalid(format!("no profile {id}")))?;
        if profile.currency != currency {
            return Err(ApiError::Invalid(format!(
                "profile {id} reads {} statements, the account is in {currency}",
                profile.currency
            )));
        }
    }
    let account = sqlx::query_as(concat!(
        "insert into accounts (household_id, name, kind, currency, namespace, profile)
         values ($1, $2, $3, $4, $5, $6) returning ",
        columns!()
    ))
    .bind(session.household_id)
    .bind(name)
    .bind(body.kind)
    .bind(currency)
    .bind(namespace)
    .bind(body.profile)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(account)))
}
