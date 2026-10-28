//! Applying the rules again to the transactions already there, in two steps: the preview lists every change and
//! tags the list (`ETag`); the apply carries that tag (`If-Match`, RFC 9110 § 13.1.1) and is refused when the list
//! is no longer the same, so what gets written is exactly what was shown. A category or payee a person set is never
//! changed.

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, header};
use serde::Serialize;
use time::Date;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};
use crate::sorting::{self, Change, PayeeChoice, Sorted};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(preview_rules))
        .routes(routes!(apply_rules))
}

/// A transaction's category and payee, and the rule that set them.
#[derive(Serialize, ToSchema)]
pub struct Sorting {
    category_id: Option<Uuid>,
    payee_id: Option<Uuid>,
    /// A payee read from the text that the household does not have yet; applying creates it.
    new_payee: Option<String>,
    rule_id: Option<Uuid>,
}

impl From<&Sorted> for Sorting {
    fn from(sorted: &Sorted) -> Sorting {
        let (payee_id, new_payee) = match &sorted.payee {
            Some(PayeeChoice::Id(id)) => (Some(*id), None),
            Some(PayeeChoice::Name(name)) => (None, Some(name.clone())),
            None => (None, None),
        };
        Sorting {
            category_id: sorted.category_id,
            payee_id,
            new_payee,
            rule_id: sorted.rule_id,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct RuleChange {
    transaction_id: Uuid,
    date: Date,
    description: String,
    /// Signed minor units in `currency`.
    amount_minor: i64,
    currency: String,
    before: Sorting,
    after: Sorting,
}

#[derive(Serialize, ToSchema)]
pub struct RulePreview {
    /// Every transaction applying the rules would change, oldest first.
    changes: Vec<RuleChange>,
    /// The same tag as the `ETag` header: send it back in `If-Match` to apply.
    tag: String,
}

#[derive(Serialize, ToSchema)]
pub struct RulesApplied {
    applied: usize,
}

fn view(changes: &[Change]) -> Vec<RuleChange> {
    changes
        .iter()
        .map(|c| RuleChange {
            transaction_id: c.transaction_id,
            date: c.date,
            description: c.description.clone(),
            amount_minor: c.amount_minor,
            currency: c.currency.clone(),
            before: Sorting::from(&c.before),
            after: Sorting::from(&c.after),
        })
        .collect()
}

/// What applying the rules again would change. Writes nothing.
#[utoipa::path(post, path = "/api/v1/rules/preview", tag = "rules",
    responses(
        (status = OK, description = "The changes, tagged with `ETag`", body = RulePreview,
            headers(("ETag" = String, description = "Tag of this list of changes"))),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn preview_rules(
    State(state): State<AppState>,
    session: Session,
) -> ApiResult<([(header::HeaderName, HeaderValue); 1], Json<RulePreview>)> {
    let mut db = state.db.acquire().await?;
    let changes = sorting::changes(&mut db, session.household_id).await?;
    let tag = sorting::entity_tag(&changes);
    let etag = HeaderValue::from_str(&tag).map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok((
        [(header::ETAG, etag)],
        Json(RulePreview {
            changes: view(&changes),
            tag,
        }),
    ))
}

/// Applies the rules again: writes exactly the changes of the preview whose tag comes in `If-Match`.
#[utoipa::path(post, path = "/api/v1/rules/apply", tag = "rules",
    params(("If-Match" = String, Header, description = "The `ETag` of the preview being applied")),
    responses(
        (status = OK, description = "How many transactions changed", body = RulesApplied),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = PRECONDITION_FAILED, description = "The changes are not the previewed ones any more", body = Problem),
        (status = PRECONDITION_REQUIRED, description = "No `If-Match`", body = Problem)))]
async fn apply_rules(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> ApiResult<Json<RulesApplied>> {
    let expected = headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::PreconditionRequired("send the preview's ETag in If-Match".to_owned()))?
        .to_owned();
    let mut tx = state.db.begin().await?;
    // One apply at a time per household; rule edits take the same lock.
    sqlx::query("select id from households where id = $1 for update")
        .bind(session.household_id)
        .execute(&mut *tx)
        .await?;
    let changes = sorting::changes(&mut tx, session.household_id).await?;
    if sorting::entity_tag(&changes) != expected.trim() {
        return Err(ApiError::PreconditionFailed(
            "the changes are not the ones previewed any more: preview again".to_owned(),
        ));
    }
    for change in &changes {
        sorting::write(&mut tx, session.household_id, change).await?;
    }
    tx.commit().await?;
    Ok(Json(RulesApplied {
        applied: changes.len(),
    }))
}
