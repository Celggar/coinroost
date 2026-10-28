//! Ordered rules that sort transactions: the first enabled rule whose conditions all hold sets the category, the
//! payee or both. Creating, changing, moving or deleting a rule changes no transaction: new imports use the rules
//! as they are, and existing transactions change only through the preview and apply in `apply.rs`.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use coinroost_rules::{PayeeAction, Rule as Check};
use serde::{Deserialize, Serialize};
use sqlx::PgConnection;
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
    OpenApiRouter::new()
        .routes(routes!(list_rules, create_rule))
        .routes(routes!(update_rule, delete_rule))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum Direction {
    In,
    Out,
}

#[derive(Clone, Deserialize, Serialize, ToSchema, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Conditions {
    /// Text the description contains, compared in upper case; kept exactly as written, spaces included.
    #[serde(default)]
    description_contains: Option<String>,
    /// `out` when the amount is below zero, `in` otherwise.
    #[serde(default)]
    direction: Option<Direction>,
    #[serde(default)]
    account_id: Option<Uuid>,
    /// Magnitude in minor units, both ends included.
    #[serde(default)]
    amount_min: Option<i64>,
    #[serde(default)]
    amount_max: Option<i64>,
}

#[derive(Clone, Deserialize, Serialize, ToSchema, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Actions {
    #[serde(default)]
    category_id: Option<Uuid>,
    #[serde(default)]
    payee_id: Option<Uuid>,
    /// The matched text is a payment channel: the payee is the name that follows it in the description.
    #[serde(default)]
    payee_from_text: bool,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewRule {
    name: String,
    /// Where it goes, 1 being first; last when absent.
    position: Option<i32>,
    #[serde(default = "enabled")]
    enabled: bool,
    when: Conditions,
    then: Actions,
}

fn enabled() -> bool {
    true
}

/// The fields to change; the others stay.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleUpdate {
    name: Option<String>,
    /// Moves the rule there; the rules in between shift by one.
    position: Option<i32>,
    enabled: Option<bool>,
    when: Option<Conditions>,
    then: Option<Actions>,
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
         category_id, payee_id, payee_from_text, created_at"
    };
}

/// The rule must be able to work; the text it looks for is kept as written.
fn check(when: &Conditions, then: &Actions) -> ApiResult<()> {
    if let Some(text) = &when.description_contains
        && text.chars().count() > 200
    {
        return Err(ApiError::Invalid(
            "description_contains has at most 200 characters".to_owned(),
        ));
    }
    let payee = match (then.payee_id, then.payee_from_text) {
        (Some(_), true) => {
            return Err(ApiError::Invalid(
                "a payee is either named or read from the text, not both".to_owned(),
            ));
        }
        (Some(id), false) => PayeeAction::Fixed(id),
        (None, true) => PayeeAction::FromText,
        (None, false) => PayeeAction::None,
    };
    let direction = when.direction.map(|d| match d {
        Direction::In => coinroost_rules::Direction::In,
        Direction::Out => coinroost_rules::Direction::Out,
    });
    Check::new(
        when.description_contains.clone(),
        direction,
        when.account_id,
        when.amount_min,
        when.amount_max,
        then.category_id,
        payee,
    )
    .map(|_| ())
    .map_err(ApiError::Invalid)
}

/// One rule change at a time per household, so positions stay a sequence; returns the number of rules.
async fn lock(db: &mut PgConnection, household_id: Uuid) -> ApiResult<i32> {
    sqlx::query("select id from households where id = $1 for update")
        .bind(household_id)
        .execute(&mut *db)
        .await?;
    sqlx::query("set constraints all deferred")
        .execute(&mut *db)
        .await?;
    let (count,): (i32,) =
        sqlx::query_as("select coalesce(max(position), 0) from rules where household_id = $1")
            .bind(household_id)
            .fetch_one(&mut *db)
            .await?;
    Ok(count)
}

/// The household's rules, first one first.
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

/// Adds a rule at a position. Transactions already there keep their category and payee.
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
    check(&body.when, &body.then)?;
    let mut tx = state.db.begin().await?;
    let last = lock(&mut tx, session.household_id).await?;
    let position = body.position.unwrap_or(last + 1);
    if !(1..=last + 1).contains(&position) {
        return Err(ApiError::Invalid(format!(
            "position must be between 1 and {}",
            last + 1
        )));
    }
    sqlx::query("update rules set position = position + 1 where household_id = $1 and position >= $2")
        .bind(session.household_id)
        .bind(position)
        .execute(&mut *tx)
        .await?;
    let (when, then) = (body.when, body.then);
    let rule = sqlx::query_as(concat!(
        "insert into rules (household_id, position, name, enabled, description_contains, direction, account_id,
                            amount_min, amount_max, category_id, payee_id, payee_from_text)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) returning ",
        columns!()
    ))
    .bind(session.household_id)
    .bind(position)
    .bind(name)
    .bind(body.enabled)
    .bind(when.description_contains)
    .bind(when.direction)
    .bind(when.account_id)
    .bind(when.amount_min)
    .bind(when.amount_max)
    .bind(then.category_id)
    .bind(then.payee_id)
    .bind(then.payee_from_text)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

/// Shifts the rules between a rule's position `from` and its new `position` (1 to `last`) to make room for it.
async fn make_room(
    tx: &mut PgConnection,
    household_id: Uuid,
    from: i32,
    position: i32,
    last: i32,
) -> ApiResult<()> {
    if !(1..=last).contains(&position) {
        return Err(ApiError::Invalid(format!(
            "position must be between 1 and {last}"
        )));
    }
    let (shift, low, high) = if position < from {
        (1, position, from - 1)
    } else {
        (-1, from + 1, position)
    };
    sqlx::query(
        "update rules set position = position + $2 where household_id = $1 and position between $3 and $4",
    )
    .bind(household_id)
    .bind(shift)
    .bind(low)
    .bind(high)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Changes a rule, or moves it. Transactions already there keep their category and payee.
#[utoipa::path(patch, path = "/api/v1/rules/{rule_id}", tag = "rules", request_body = RuleUpdate,
    params(("rule_id" = Uuid, Path, description = "The rule")),
    responses(
        (status = OK, description = "The rule as it is now", body = Rule),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such rule", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "A value is not valid or refers to nothing", body = Problem)))]
async fn update_rule(
    State(state): State<AppState>,
    session: Session,
    Path(rule_id): Path<Uuid>,
    Json(body): Json<RuleUpdate>,
) -> ApiResult<Json<Rule>> {
    let mut tx = state.db.begin().await?;
    let last = lock(&mut tx, session.household_id).await?;
    let current: Rule = sqlx::query_as(concat!(
        "select ",
        columns!(),
        " from rules where id = $1 and household_id = $2"
    ))
    .bind(rule_id)
    .bind(session.household_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::NotFound)?;
    let name = match &body.name {
        Some(name) => validate::text("name", name, 200)?,
        None => current.name,
    };
    let when = body.when.unwrap_or(current.when);
    let then = body.then.unwrap_or(current.then);
    check(&when, &then)?;
    if let Some(position) = body.position {
        make_room(&mut tx, session.household_id, current.position, position, last).await?;
    }
    let rule = sqlx::query_as(concat!(
        "update rules set position = $3, name = $4, enabled = $5, description_contains = $6, direction = $7,
                          account_id = $8, amount_min = $9, amount_max = $10, category_id = $11, payee_id = $12,
                          payee_from_text = $13
         where id = $1 and household_id = $2 returning ",
        columns!()
    ))
    .bind(rule_id)
    .bind(session.household_id)
    .bind(body.position.unwrap_or(current.position))
    .bind(name)
    .bind(body.enabled.unwrap_or(current.enabled))
    .bind(when.description_contains)
    .bind(when.direction)
    .bind(when.account_id)
    .bind(when.amount_min)
    .bind(when.amount_max)
    .bind(then.category_id)
    .bind(then.payee_id)
    .bind(then.payee_from_text)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(rule))
}

/// Deletes a rule; the ones after it move up. Transactions it sorted keep their category and payee.
#[utoipa::path(delete, path = "/api/v1/rules/{rule_id}", tag = "rules",
    params(("rule_id" = Uuid, Path, description = "The rule")),
    responses(
        (status = NO_CONTENT, description = "Deleted"),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such rule", body = Problem)))]
async fn delete_rule(
    State(state): State<AppState>,
    session: Session,
    Path(rule_id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    let mut tx = state.db.begin().await?;
    lock(&mut tx, session.household_id).await?;
    let deleted: Option<(i32,)> =
        sqlx::query_as("delete from rules where id = $1 and household_id = $2 returning position")
            .bind(rule_id)
            .bind(session.household_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (position,) = deleted.ok_or(ApiError::NotFound)?;
    sqlx::query("update rules set position = position - 1 where household_id = $1 and position > $2")
        .bind(session.household_id)
        .bind(position)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
