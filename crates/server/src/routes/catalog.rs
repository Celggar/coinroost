//! Categories and payees: the names transactions are sorted under. Both are a household's list of unique names.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiResult, Problem};
use crate::validate;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list_categories, create_category))
        .routes(routes!(list_payees, create_payee))
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewName {
    name: String,
}

/// A category or a payee.
#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Named {
    id: Uuid,
    name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
pub struct NamedList {
    items: Vec<Named>,
}

/// The two tables this module serves; every statement is a fixed string.
#[derive(Clone, Copy)]
enum Table {
    Categories,
    Payees,
}

impl Table {
    fn select(self) -> &'static str {
        match self {
            Table::Categories => {
                "select id, name, created_at from categories where household_id = $1 order by name"
            }
            Table::Payees => "select id, name, created_at from payees where household_id = $1 order by name",
        }
    }

    fn insert(self) -> &'static str {
        match self {
            Table::Categories => {
                "insert into categories (household_id, name) values ($1, $2) returning id, name, created_at"
            }
            Table::Payees => {
                "insert into payees (household_id, name) values ($1, $2) returning id, name, created_at"
            }
        }
    }

    fn max_length(self) -> usize {
        match self {
            Table::Categories => 100,
            Table::Payees => 200,
        }
    }
}

async fn list(state: &AppState, session: Session, table: Table) -> ApiResult<Json<NamedList>> {
    let items = sqlx::query_as(table.select())
        .bind(session.household_id)
        .fetch_all(&state.db)
        .await?;
    Ok(Json(NamedList { items }))
}

async fn create(
    state: &AppState,
    session: Session,
    table: Table,
    body: NewName,
) -> ApiResult<(StatusCode, Json<Named>)> {
    let name = validate::text("name", &body.name, table.max_length())?;
    let row = sqlx::query_as(table.insert())
        .bind(session.household_id)
        .bind(name)
        .fetch_one(&state.db)
        .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// The household's categories, by name.
#[utoipa::path(get, path = "/api/v1/categories", tag = "categories",
    responses(
        (status = OK, description = "The categories", body = NamedList),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_categories(State(state): State<AppState>, session: Session) -> ApiResult<Json<NamedList>> {
    list(&state, session, Table::Categories).await
}

/// Adds a category.
#[utoipa::path(post, path = "/api/v1/categories", tag = "categories", request_body = NewName,
    responses(
        (status = CREATED, description = "The new category", body = Named),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The name is taken", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The name is empty or too long", body = Problem)))]
async fn create_category(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewName>,
) -> ApiResult<(StatusCode, Json<Named>)> {
    create(&state, session, Table::Categories, body).await
}

/// The household's payees, by name.
#[utoipa::path(get, path = "/api/v1/payees", tag = "payees",
    responses(
        (status = OK, description = "The payees", body = NamedList),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_payees(State(state): State<AppState>, session: Session) -> ApiResult<Json<NamedList>> {
    list(&state, session, Table::Payees).await
}

/// Adds a payee.
#[utoipa::path(post, path = "/api/v1/payees", tag = "payees", request_body = NewName,
    responses(
        (status = CREATED, description = "The new payee", body = Named),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The name is taken", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The name is empty or too long", body = Problem)))]
async fn create_payee(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewName>,
) -> ApiResult<(StatusCode, Json<Named>)> {
    create(&state, session, Table::Payees, body).await
}
