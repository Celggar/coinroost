//! Transfers between own accounts. Imports pair what is certain on their own; these routes show what is waiting
//! (`preview`, tagged with an `ETag`), pair it (`apply`, with `If-Match`), pair two transactions a person chose,
//! and split a transfer back into its two transactions.

use std::collections::HashSet;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};
use crate::pairing::{self, Candidate, Planned};

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(preview_transfers))
        .routes(routes!(apply_transfers))
        .routes(routes!(pair_transactions))
        .routes(routes!(split_transfer))
}

/// One leg that may be half of a transfer, with its transaction.
#[derive(Serialize, ToSchema)]
pub struct TransferLeg {
    transaction_id: Uuid,
    date: Date,
    description: String,
    /// Signed minor units in `currency`: below zero the money left `account_id`.
    amount_minor: i64,
    currency: String,
    account_id: Uuid,
    category_id: Option<Uuid>,
}

impl From<&Candidate> for TransferLeg {
    fn from(c: &Candidate) -> TransferLeg {
        TransferLeg {
            transaction_id: c.transaction_id,
            date: c.date,
            description: c.description.clone(),
            amount_minor: c.amount_minor,
            currency: c.currency.clone(),
            account_id: c.account_id,
            category_id: c.category_id,
        }
    }
}

#[derive(Serialize, ToSchema)]
pub struct TransferPair {
    outflow: TransferLeg,
    inflow: TransferLeg,
    /// 1: the only match within two days; 2: the outflow's text names the account; 3: Friday to Monday.
    pass: u8,
    /// Pairing drops a category or payee a person chose for the inflow, so imports leave it for you.
    drops_manual: bool,
}

/// Why a leg is not paired.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// No leg of the other side within two days: its statement may not be imported yet.
    NoCounterpart,
    /// More than one leg could be the other side; nothing is guessed.
    SeveralCandidates,
}

#[derive(Serialize, ToSchema)]
pub struct Unmatched {
    leg: TransferLeg,
    reason: Reason,
    /// Unpaired legs that could be its other side: same amount, other sign, other account, same currency (any
    /// date when the reason is `no_counterpart`).
    candidates: Vec<TransferLeg>,
}

#[derive(Serialize, ToSchema)]
pub struct TransferPreview {
    /// The pairs applying would make, oldest first.
    pairs: Vec<TransferPair>,
    unmatched: Vec<Unmatched>,
    /// The same tag as the `ETag` header: send it back in `If-Match` to apply.
    tag: String,
}

#[derive(Serialize, ToSchema)]
pub struct TransfersApplied {
    paired: usize,
}

fn view(planned: &Planned) -> (Vec<TransferPair>, Vec<Unmatched>) {
    let pool = &planned.pool;
    let pairs: Vec<TransferPair> = planned
        .plan
        .pairs
        .iter()
        .map(|p| TransferPair {
            outflow: TransferLeg::from(&pool[p.outflow]),
            inflow: TransferLeg::from(&pool[p.inflow]),
            pass: pairing::pass_number(p.pass),
            drops_manual: planned.drops_manual(p.outflow, p.inflow),
        })
        .collect();
    let paired: HashSet<usize> = planned
        .plan
        .pairs
        .iter()
        .flat_map(|p| [p.outflow, p.inflow])
        .collect();
    let counterpart = |a: &Candidate, b: &Candidate| {
        a.amount_minor.checked_neg() == Some(b.amount_minor)
            && a.account_id != b.account_id
            && a.currency == b.currency
    };
    let mut unmatched: Vec<(usize, Reason, Vec<usize>)> = planned
        .plan
        .orphans
        .iter()
        .map(|&i| {
            let any_date = (0..pool.len())
                .filter(|&j| !paired.contains(&j) && counterpart(&pool[i], &pool[j]))
                .collect();
            (i, Reason::NoCounterpart, any_date)
        })
        .chain(planned.plan.ambiguous.iter().map(|(i, found)| {
            let left = found.iter().copied().filter(|j| !paired.contains(j)).collect();
            (*i, Reason::SeveralCandidates, left)
        }))
        .collect();
    unmatched.sort_by_key(|(i, ..)| (pool[*i].date, pool[*i].transaction_id));
    let unmatched = unmatched
        .into_iter()
        .map(|(i, reason, found)| Unmatched {
            leg: TransferLeg::from(&pool[i]),
            reason,
            candidates: found.iter().map(|&j| TransferLeg::from(&pool[j])).collect(),
        })
        .collect();
    (pairs, unmatched)
}

/// What pairing would do now, over every unpaired leg whose category is a transfer. Writes nothing.
#[utoipa::path(post, path = "/api/v1/transfers/preview", tag = "transfers",
    responses(
        (status = OK, description = "The pairs waiting and the legs left unpaired, tagged with `ETag`",
            body = TransferPreview,
            headers(("ETag" = String, description = "Tag of these pairs"))),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn preview_transfers(
    State(state): State<AppState>,
    session: Session,
) -> ApiResult<([(header::HeaderName, HeaderValue); 1], Json<TransferPreview>)> {
    let mut db = state.db.acquire().await?;
    let planned = pairing::planned(&mut db, session.household_id).await?;
    let tag = pairing::entity_tag(&planned);
    let etag = HeaderValue::from_str(&tag).map_err(|e| ApiError::Internal(e.to_string()))?;
    let (pairs, unmatched) = view(&planned);
    Ok((
        [(header::ETAG, etag)],
        Json(TransferPreview {
            pairs,
            unmatched,
            tag,
        }),
    ))
}

/// Makes exactly the pairs of the preview whose tag comes in `If-Match`.
#[utoipa::path(post, path = "/api/v1/transfers/apply", tag = "transfers",
    params(("If-Match" = String, Header, description = "The `ETag` of the preview being applied")),
    responses(
        (status = OK, description = "How many pairs were made", body = TransfersApplied),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = PRECONDITION_FAILED, description = "The pairs are not the previewed ones any more", body = Problem),
        (status = PRECONDITION_REQUIRED, description = "No `If-Match`", body = Problem)))]
async fn apply_transfers(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> ApiResult<Json<TransfersApplied>> {
    let expected = headers
        .get(header::IF_MATCH)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::PreconditionRequired("send the preview's ETag in If-Match".to_owned()))?
        .to_owned();
    let mut tx = state.db.begin().await?;
    pairing::lock(&mut tx, session.household_id).await?;
    let planned = pairing::planned(&mut tx, session.household_id).await?;
    if pairing::entity_tag(&planned) != expected.trim() {
        return Err(ApiError::PreconditionFailed(
            "the pairs are not the ones previewed any more: preview again".to_owned(),
        ));
    }
    for pair in &planned.plan.pairs {
        let (out, inn) = (&planned.pool[pair.outflow], &planned.pool[pair.inflow]);
        pairing::merge(
            &mut tx,
            session.household_id,
            out.transaction_id,
            inn.transaction_id,
        )
        .await?;
    }
    tx.commit().await?;
    Ok(Json(TransfersApplied {
        paired: planned.plan.pairs.len(),
    }))
}

/// Two one-leg transactions a person says are the same movement.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PairRequest {
    transaction_ids: [Uuid; 2],
}

#[derive(Serialize, ToSchema)]
pub struct Paired {
    /// The transfer: the outflow's transaction, now with both legs.
    transaction_id: Uuid,
}

/// Pairs two one-leg transactions into one transfer, whatever their dates and categories.
#[utoipa::path(post, path = "/api/v1/transfers", tag = "transfers", request_body = PairRequest,
    responses(
        (status = CREATED, description = "The transfer", body = Paired),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The two cannot be one transfer; the detail says why",
            body = Problem)))]
async fn pair_transactions(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<PairRequest>,
) -> ApiResult<(StatusCode, Json<Paired>)> {
    let [a, b] = body.transaction_ids;
    if a == b {
        return Err(ApiError::Invalid("choose two different transactions".to_owned()));
    }
    let mut tx = state.db.begin().await?;
    pairing::lock(&mut tx, session.household_id).await?;
    let found = pairing::single(&mut tx, session.household_id, &[a, b]).await?;
    let [x, y] = found.as_slice() else {
        return Err(ApiError::Invalid(
            "both must be transactions of this household with one leg each".to_owned(),
        ));
    };
    let (out, inn) = if x.is_outflow() { (x, y) } else { (y, x) };
    if out.account_id == inn.account_id {
        return Err(ApiError::Invalid(
            "a transfer moves money between two different accounts".to_owned(),
        ));
    }
    if out.currency != inn.currency {
        return Err(ApiError::Invalid(
            "a transfer moves money within one currency".to_owned(),
        ));
    }
    if out.amount_minor.checked_add(inn.amount_minor) != Some(0) || !out.is_outflow() {
        return Err(ApiError::Invalid(
            "the two legs of a transfer sum to zero: one leaves an account, the other arrives".to_owned(),
        ));
    }
    pairing::merge(
        &mut tx,
        session.household_id,
        out.transaction_id,
        inn.transaction_id,
    )
    .await?;
    tx.commit().await?;
    Ok((
        StatusCode::CREATED,
        Json(Paired {
            transaction_id: out.transaction_id,
        }),
    ))
}

#[derive(Serialize, ToSchema)]
pub struct Split {
    /// The transfer's transaction, now with the outflow leg only, then the new one with the inflow leg.
    transaction_ids: [Uuid; 2],
}

/// Splits a transfer back into two one-leg transactions. Balances do not change.
#[utoipa::path(post, path = "/api/v1/transactions/{transaction_id}/split", tag = "transfers",
    params(("transaction_id" = Uuid, Path, description = "A transaction with two legs")),
    responses(
        (status = OK, description = "The two transactions", body = Split),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such transaction", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "It does not have two legs", body = Problem)))]
async fn split_transfer(
    State(state): State<AppState>,
    session: Session,
    Path(transaction_id): Path<Uuid>,
) -> ApiResult<Json<Split>> {
    let mut tx = state.db.begin().await?;
    pairing::lock(&mut tx, session.household_id).await?;
    let inflow = pairing::split(&mut tx, session.household_id, transaction_id).await?;
    tx.commit().await?;
    Ok(Json(Split {
        transaction_ids: [transaction_id, inflow],
    }))
}
