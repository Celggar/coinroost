//! Categories and payees: the names transactions are sorted under. Both are a household's list of unique names;
//! a category also says what it holds.

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

/// What a category holds: spending, income, or money moved between the household's own accounts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum CategoryKind {
    #[default]
    Expense,
    Income,
    Transfer,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewCategory {
    name: String,
    /// `expense` when absent.
    #[serde(default)]
    kind: CategoryKind,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Category {
    id: Uuid,
    name: String,
    kind: CategoryKind,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
pub struct Categories {
    items: Vec<Category>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewName {
    name: String,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Payee {
    id: Uuid,
    name: String,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
pub struct Payees {
    items: Vec<Payee>,
}

/// The household's categories, by name.
#[utoipa::path(get, path = "/api/v1/categories", tag = "categories",
    responses(
        (status = OK, description = "The categories", body = Categories),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_categories(State(state): State<AppState>, session: Session) -> ApiResult<Json<Categories>> {
    let items = sqlx::query_as(
        "select id, name, kind, created_at from categories where household_id = $1 order by name",
    )
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Categories { items }))
}

/// Adds a category.
#[utoipa::path(post, path = "/api/v1/categories", tag = "categories", request_body = NewCategory,
    responses(
        (status = CREATED, description = "The new category", body = Category),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The name is taken", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The name is empty or too long", body = Problem)))]
async fn create_category(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewCategory>,
) -> ApiResult<(StatusCode, Json<Category>)> {
    let name = validate::text("name", &body.name, 100)?;
    let row = sqlx::query_as(
        "insert into categories (household_id, name, kind) values ($1, $2, $3)
         returning id, name, kind, created_at",
    )
    .bind(session.household_id)
    .bind(name)
    .bind(body.kind)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

/// The household's payees, by name.
#[utoipa::path(get, path = "/api/v1/payees", tag = "payees",
    responses(
        (status = OK, description = "The payees", body = Payees),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_payees(State(state): State<AppState>, session: Session) -> ApiResult<Json<Payees>> {
    let items =
        sqlx::query_as("select id, name, created_at from payees where household_id = $1 order by name")
            .bind(session.household_id)
            .fetch_all(&state.db)
            .await?;
    Ok(Json(Payees { items }))
}

/// Adds a payee.
#[utoipa::path(post, path = "/api/v1/payees", tag = "payees", request_body = NewName,
    responses(
        (status = CREATED, description = "The new payee", body = Payee),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The name is taken", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The name is empty or too long", body = Problem)))]
async fn create_payee(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewName>,
) -> ApiResult<(StatusCode, Json<Payee>)> {
    let name = validate::text("name", &body.name, 200)?;
    let row = sqlx::query_as(
        "insert into payees (household_id, name) values ($1, $2) returning id, name, created_at",
    )
    .bind(session.household_id)
    .bind(name)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}
