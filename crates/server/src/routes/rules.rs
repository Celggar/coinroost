//! Ordered rules that sort transactions: the first enabled rule whose conditions all hold sets the category, the
//! payee or both. Applying them is a later step; here they are stored, in order.

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
use crate::error::{ApiError, ApiResult, Problem};
use crate::validate;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list_rules, create_rule))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum Direction {
    /// Money coming into the account.
    In,
    /// Money leaving the account.
    Out,
}

/// Conditions; every one that is set must hold, and at least one must be set.
#[derive(Deserialize, Serialize, ToSchema, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    /// Case-insensitive text the description must contain.
    #[serde(default)]
    description_contains: Option<String>,
    #[serde(default)]
    direction: Option<Direction>,
    #[serde(default)]
    account_id: Option<Uuid>,
    /// Smallest magnitude, in minor units, included.
    #[serde(default)]
    amount_min: Option<i64>,
    /// Largest magnitude, in minor units, included.
    #[serde(default)]
    amount_max: Option<i64>,
}

/// What a matching rule sets; at least one.
#[derive(Deserialize, Serialize, ToSchema, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Actions {
    #[serde(default)]
    category_id: Option<Uuid>,
    #[serde(default)]
    payee_id: Option<Uuid>,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewRule {
    name: String,
    /// Where it goes, 1 being first; the rules from there on move down one. Last when absent.
    position: Option<i32>,
    #[serde(default = "enabled")]
    enabled: bool,
    when: Conditions,
    then: Actions,
}

fn enabled() -> bool {
    true
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Rule {
    id: Uuid,
    position: i32,
    name: String,
    enabled: bool,
    #[sqlx(flatten)]
    when: Conditions,
    #[sqlx(flatten)]
    then: Actions,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
pub struct Rules {
    items: Vec<Rule>,
}

macro_rules! columns {
    () => {
        "id, position, name, enabled, description_contains, direction, account_id, amount_min, amount_max, \
         category_id, payee_id, created_at"
    };
}

/// The household's rules, in the order they are tried.
#[utoipa::path(get, path = "/api/v1/rules", tag = "rules",
    responses(
        (status = OK, description = "The rules, first one first", body = Rules),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_rules(State(state): State<AppState>, session: Session) -> ApiResult<Json<Rules>> {
    let items = sqlx::query_as(concat!(
        "select ",
        columns!(),
        " from rules where household_id = $1 order by position"
    ))
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Rules { items }))
}

/// Adds a rule at a position.
#[utoipa::path(post, path = "/api/v1/rules", tag = "rules", request_body = NewRule,
    responses(
        (status = CREATED, description = "The new rule", body = Rule),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "A value is not valid or refers to nothing", body = Problem)))]
async fn create_rule(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewRule>,
) -> ApiResult<(StatusCode, Json<Rule>)> {
    let name = validate::text("name", &body.name, 200)?;
    let when = body.when;
    let contains = when
        .description_contains
        .as_deref()
        .map(|text| validate::text("description_contains", text, 200))
        .transpose()?;
    let any_condition = contains.is_some()
        || when.direction.is_some()
        || when.account_id.is_some()
        || when.amount_min.is_some()
        || when.amount_max.is_some();
    if !any_condition {
        return Err(ApiError::Invalid(
            "a rule needs at least one condition".to_owned(),
        ));
    }
    if when.amount_min.is_some_and(|v| v < 0) || when.amount_max.is_some_and(|v| v < 0) {
        return Err(ApiError::Invalid(
            "amounts in a rule are magnitudes, 0 or more".to_owned(),
        ));
    }
    if let (Some(min), Some(max)) = (when.amount_min, when.amount_max)
        && min > max
    {
        return Err(ApiError::Invalid("amount_min is above amount_max".to_owned()));
    }
    if body.then.category_id.is_none() && body.then.payee_id.is_none() {
        return Err(ApiError::Invalid(
            "a rule sets a category, a payee or both".to_owned(),
        ));
    }

    let mut tx = state.db.begin().await?;
    // One rule change at a time per household, so positions stay a sequence.
    sqlx::query("select id from households where id = $1 for update")
        .bind(session.household_id)
        .execute(&mut *tx)
        .await?;
    let (last,): (i32,) =
        sqlx::query_as("select coalesce(max(position), 0) from rules where household_id = $1")
            .bind(session.household_id)
            .fetch_one(&mut *tx)
            .await?;
    let position = body.position.unwrap_or(last + 1);
    if !(1..=last + 1).contains(&position) {
        return Err(ApiError::Invalid(format!(
            "position must be between 1 and {}",
            last + 1
        )));
    }
    sqlx::query("set constraints all deferred")
        .execute(&mut *tx)
        .await?;
    sqlx::query("update rules set position = position + 1 where household_id = $1 and position >= $2")
        .bind(session.household_id)
        .bind(position)
        .execute(&mut *tx)
        .await?;
    let rule = sqlx::query_as(concat!(
        "insert into rules (household_id, position, name, enabled, description_contains, direction, account_id,
                            amount_min, amount_max, category_id, payee_id)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) returning ",
        columns!()
    ))
    .bind(session.household_id)
    .bind(position)
    .bind(name)
    .bind(body.enabled)
    .bind(contains)
    .bind(when.direction)
    .bind(when.account_id)
    .bind(when.amount_min)
    .bind(when.amount_max)
    .bind(body.then.category_id)
    .bind(body.then.payee_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(rule)))
}
