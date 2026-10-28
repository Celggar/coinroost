//! The household's transfer legs, run through `coinroost-transfers`. A candidate is a one-leg transaction whose
//! category is of kind transfer. Pairing moves the inflow's leg into the outflow's transaction and deletes the
//! inflow's now empty transaction: the same two legs exist before and after, so no balance changes, and both keep
//! their `external_id`, so importing either statement again adds nothing. The transfer keeps the outflow's date,
//! description, category, payee and rule. Splitting gives the inflow its own transaction back.

use std::fmt::Write;

use coinroost_transfers::{Leg, Pass, Plan, plan};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use time::{Date, Duration};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

/// Transfers made this long before the oldest candidate still teach which account a number in a text names:
/// a monthly import alone is too short to see the recurring ones twice.
const EVIDENCE_DAYS: i64 = 400;

/// A one-leg transaction that may be half of a transfer.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Candidate {
    pub transaction_id: Uuid,
    /// The leg's date, as the bank printed it.
    pub date: Date,
    pub description: String,
    pub amount_minor: i64,
    pub currency: String,
    pub account_id: Uuid,
    pub statement_text: String,
    pub import_batch_id: Option<Uuid>,
    pub category_id: Option<Uuid>,
    pub payee_id: Option<Uuid>,
    /// A person chose its category or payee.
    pub manual: bool,
}

impl Candidate {
    pub fn is_outflow(&self) -> bool {
        self.amount_minor < 0
    }
}

/// The plan for the household, over every unpaired candidate it has.
pub struct Planned {
    pub pool: Vec<Candidate>,
    pub plan: Plan,
}

impl Planned {
    /// Pairing `outflow` with `inflow` would drop a category or payee a person chose for the inflow.
    pub fn drops_manual(&self, outflow: usize, inflow: usize) -> bool {
        let (out, inn) = (&self.pool[outflow], &self.pool[inflow]);
        inn.manual && (inn.category_id != out.category_id || inn.payee_id != out.payee_id)
    }
}

macro_rules! candidate_columns {
    () => {
        "select t.id as transaction_id, l.posted_on as date, t.description, l.amount_minor, l.currency, l.account_id,
                l.statement_text, l.import_batch_id, t.category_id, t.payee_id,
                (t.category_set_by = 'manual' or t.payee_set_by = 'manual') is true as manual
         from transactions t join legs l on l.transaction_id = t.id "
    };
}

/// Every unpaired candidate of the household, by date.
pub async fn pool(db: &mut PgConnection, household_id: Uuid) -> ApiResult<Vec<Candidate>> {
    Ok(sqlx::query_as(concat!(
        candidate_columns!(),
        "join categories c on c.id = t.category_id
         where t.household_id = $1 and c.kind = 'transfer'
           and (select count(*) from legs o where o.transaction_id = t.id) = 1
         order by l.posted_on, t.id"
    ))
    .bind(household_id)
    .fetch_all(db)
    .await?)
}

/// One-leg transactions of the household by id, whatever their category.
pub async fn single(db: &mut PgConnection, household_id: Uuid, ids: &[Uuid]) -> ApiResult<Vec<Candidate>> {
    Ok(sqlx::query_as(concat!(
        candidate_columns!(),
        "where t.household_id = $1 and t.id = any($2)
           and (select count(*) from legs o where o.transaction_id = t.id) = 1
         order by t.id"
    ))
    .bind(household_id)
    .bind(ids)
    .fetch_all(db)
    .await?)
}

/// What pairing would do now.
pub async fn planned(db: &mut PgConnection, household_id: Uuid) -> ApiResult<Planned> {
    let pool = pool(&mut *db, household_id).await?;
    let Some(oldest) = pool.iter().map(|c| c.date).min() else {
        return Ok(Planned {
            pool,
            plan: Plan::default(),
        });
    };
    let evidence: Vec<(String, Uuid)> = sqlx::query_as(
        "select o.statement_text, i.account_id
         from transactions t
         join legs o on o.transaction_id = t.id and o.amount_minor < 0
         join legs i on i.transaction_id = t.id and i.amount_minor > 0
         where t.household_id = $1 and t.date >= $2",
    )
    .bind(household_id)
    .bind(oldest - Duration::days(EVIDENCE_DAYS))
    .fetch_all(&mut *db)
    .await?;
    let legs: Vec<Leg<Uuid, Uuid>> = pool
        .iter()
        .map(|c| Leg {
            id: c.transaction_id,
            date: c.date,
            amount_minor: c.amount_minor,
            currency: c.currency.clone(),
            account: c.account_id,
            text: c.statement_text.clone(),
        })
        .collect();
    let known: Vec<(&str, Uuid)> = evidence
        .iter()
        .map(|(text, account)| (text.as_str(), *account))
        .collect();
    let plan = plan(&legs, &known);
    Ok(Planned { pool, plan })
}

/// A strong validator (RFC 9110 § 8.8.3) of the pairs a plan would make: equal pairs give equal tags.
pub fn entity_tag(planned: &Planned) -> String {
    let mut hash = Sha256::new();
    for pair in &planned.plan.pairs {
        let (out, inn) = (&planned.pool[pair.outflow], &planned.pool[pair.inflow]);
        hash.update(format!(
            "{}|{}|{}\n",
            out.transaction_id, inn.transaction_id, pair.pass as u8
        ));
    }
    let hex = hash
        .finalize()
        .iter()
        .take(16)
        .fold(String::with_capacity(32), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        });
    format!("\"{hex}\"")
}

/// Serializes pairing, rule edits and applies within a household (they all take this lock first or after an
/// account's, never before one).
pub async fn lock(db: &mut PgConnection, household_id: Uuid) -> ApiResult<()> {
    sqlx::query("select id from households where id = $1 for update")
        .bind(household_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Moves the inflow's leg into the outflow's transaction. Both must still have one leg each.
pub async fn merge(db: &mut PgConnection, household_id: Uuid, outflow: Uuid, inflow: Uuid) -> ApiResult<()> {
    let moved = sqlx::query(
        "update legs set transaction_id = $1
         where transaction_id = $2 and household_id = $3
           and (select count(*) from legs o where o.transaction_id = $1) = 1
           and (select count(*) from legs i where i.transaction_id = $2) = 1",
    )
    .bind(outflow)
    .bind(inflow)
    .bind(household_id)
    .execute(&mut *db)
    .await?
    .rows_affected();
    if moved != 1 {
        return Err(ApiError::Conflict(
            "one of the two is not a single leg any more".to_owned(),
        ));
    }
    sqlx::query("delete from transactions where id = $1 and household_id = $2")
        .bind(inflow)
        .bind(household_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Pairs what a new import made certain: the pairs of the plan that hold a leg of `batch_id`, except those that
/// would drop a category or payee a person chose (they wait in the preview). Returns how many.
pub async fn pair_imported(db: &mut PgConnection, household_id: Uuid, batch_id: Uuid) -> ApiResult<usize> {
    lock(&mut *db, household_id).await?;
    let planned = planned(&mut *db, household_id).await?;
    let mut count = 0;
    for pair in &planned.plan.pairs {
        let (out, inn) = (&planned.pool[pair.outflow], &planned.pool[pair.inflow]);
        let new = [out, inn].iter().any(|c| c.import_batch_id == Some(batch_id));
        if new && !planned.drops_manual(pair.outflow, pair.inflow) {
            merge(&mut *db, household_id, out.transaction_id, inn.transaction_id).await?;
            count += 1;
        }
    }
    Ok(count)
}

/// Gives the inflow leg of a two-leg transfer its own transaction again, dated and described as the bank printed
/// it, with the transfer's category. Returns the new transaction's id.
pub async fn split(db: &mut PgConnection, household_id: Uuid, transaction_id: Uuid) -> ApiResult<Uuid> {
    let legs: Vec<(Uuid, i64, Date, String)> = sqlx::query_as(
        "select id, amount_minor, posted_on, statement_text from legs
         where transaction_id = $1 and household_id = $2 order by amount_minor",
    )
    .bind(transaction_id)
    .bind(household_id)
    .fetch_all(&mut *db)
    .await?;
    let [_, (leg_id, _, posted_on, text)] = legs.as_slice() else {
        return Err(if legs.is_empty() {
            ApiError::NotFound
        } else {
            ApiError::Invalid("only a transfer of two legs splits".to_owned())
        });
    };
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into transactions (household_id, date, description, category_id, category_set_by, rule_id)
         select household_id, $3, case when $4 = '' then description else $4 end, category_id, category_set_by,
                case when category_set_by = 'rule' then rule_id end
         from transactions where id = $1 and household_id = $2
         returning id",
    )
    .bind(transaction_id)
    .bind(household_id)
    .bind(posted_on)
    .bind(text)
    .fetch_one(&mut *db)
    .await?;
    sqlx::query("update legs set transaction_id = $1 where id = $2")
        .bind(id)
        .bind(leg_id)
        .execute(db)
        .await?;
    Ok(id)
}

/// The pass number shown to people.
pub fn pass_number(pass: Pass) -> u8 {
    pass as u8
}
