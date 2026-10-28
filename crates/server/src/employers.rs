//! The household's employers, loaded from a file by name, and what each still owes through `coinroost-payroll`.
//! A deposit is an inflow of a one-leg transaction on the employer's account; the account's last data is its
//! latest leg, so a fortnight due after the last imported statement is unknown, never owed.

use std::sync::LazyLock;

use coinroost_payroll::{Deposit, Employer, Fortnight, Labels, NoDeposits, Report, report};
use serde::Deserialize;
use sqlx::PgPool;
use time::{Date, Month};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};

static LABELS: LazyLock<Option<Labels>> = LazyLock::new(|| Labels::builtin("es-PA"));

/// The employers file: `{"employers": [{"name", "match", "account", "min_amount_minor", "amount_minor", "start"}]}`,
/// the account named by its namespace and the start as `YYYY-MM`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmployersFile {
    employers: Vec<NewEmployer>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewEmployer {
    name: String,
    #[serde(rename = "match")]
    match_text: String,
    account: String,
    min_amount_minor: i64,
    amount_minor: i64,
    start: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    pub added: usize,
    pub changed: usize,
}

impl EmployersFile {
    pub fn from_json(text: &str) -> Result<EmployersFile, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}

fn start_month(text: &str) -> Option<Date> {
    let (year, month) = text.split_once('-')?;
    let month = Month::try_from(month.parse::<u8>().ok()?).ok()?;
    Date::from_calendar_date(year.parse().ok()?, month, 1).ok()
}

/// Adds the employers the household does not have and updates the others, by name. Loading the same file again
/// changes nothing.
pub async fn load(db: &PgPool, household_id: Uuid, file: &EmployersFile) -> Result<LoadReport, String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    let mut report = LoadReport::default();
    for e in &file.employers {
        let name = e.name.trim();
        if name.is_empty() || e.match_text.trim().is_empty() || e.min_amount_minor <= 0 || e.amount_minor <= 0
        {
            return Err(format!(
                "employer {name:?}: a name, a match text and positive amounts are needed"
            ));
        }
        let start = start_month(&e.start).ok_or_else(|| format!("employer {name:?}: start is YYYY-MM"))?;
        let account: Option<(Uuid,)> =
            sqlx::query_as("select id from accounts where household_id = $1 and namespace = $2")
                .bind(household_id)
                .bind(&e.account)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
        let (account_id,) =
            account.ok_or_else(|| format!("employer {name:?}: no account {:?}", e.account))?;
        let row = (
            e.match_text.trim(),
            account_id,
            e.min_amount_minor,
            e.amount_minor,
            start,
        );
        let existing: Option<(Uuid, String, Uuid, i64, i64, Date)> = sqlx::query_as(
            "select id, match_text, account_id, min_amount_minor, amount_minor, start_month
             from employers where household_id = $1 and name = $2",
        )
        .bind(household_id)
        .bind(name)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
        let query = match &existing {
            None => {
                report.added += 1;
                "insert into employers (household_id, name, match_text, account_id, min_amount_minor, amount_minor,
                                        start_month)
                 values ($1, $2, $3, $4, $5, $6, $7)"
            }
            Some((_, m, a, min, amount, s)) if (m.as_str(), *a, *min, *amount, *s) == row => continue,
            Some(_) => {
                report.changed += 1;
                "update employers set match_text = $3, account_id = $4, min_amount_minor = $5, amount_minor = $6,
                                      start_month = $7
                 where household_id = $1 and name = $2"
            }
        };
        sqlx::query(query)
            .bind(household_id)
            .bind(name)
            .bind(row.0)
            .bind(row.1)
            .bind(row.2)
            .bind(row.3)
            .bind(row.4)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
    }
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(report)
}

/// One stored employer.
#[derive(sqlx::FromRow)]
pub struct Stored {
    pub id: Uuid,
    pub name: String,
    pub match_text: String,
    pub account_id: Uuid,
    pub currency: String,
    pub min_amount_minor: i64,
    pub amount_minor: i64,
    pub start_month: Date,
}

/// A deposit as the report shows it.
#[derive(sqlx::FromRow)]
pub struct Paid {
    pub transaction_id: Uuid,
    pub date: Date,
    pub amount_minor: i64,
    pub description: String,
}

/// What one employer owes, or why it could not be worked out.
pub struct Owing {
    pub employer: Stored,
    pub outcome: Result<(Report, Vec<Paid>), NoDeposits>,
}

/// Every employer of the household with its report at `as_of`, by name.
pub async fn owing(db: &PgPool, household_id: Uuid, as_of: Date) -> ApiResult<Vec<Owing>> {
    let labels = LABELS
        .as_ref()
        .ok_or_else(|| ApiError::Internal("the payroll words do not load".to_owned()))?;
    let employers: Vec<Stored> = sqlx::query_as(
        "select e.id, e.name, e.match_text, e.account_id, a.currency, e.min_amount_minor, e.amount_minor, e.start_month
         from employers e join accounts a on a.id = e.account_id
         where e.household_id = $1 order by e.name",
    )
    .bind(household_id)
    .fetch_all(db)
    .await?;
    let mut found = Vec::with_capacity(employers.len());
    for employer in employers {
        let paid: Vec<Paid> = sqlx::query_as(
            "select t.id as transaction_id, l.posted_on as date, l.amount_minor, t.description
             from transactions t join legs l on l.transaction_id = t.id
             where t.household_id = $1 and l.account_id = $2 and l.amount_minor > 0
               and (select count(*) from legs o where o.transaction_id = t.id) = 1
             order by l.posted_on, t.id",
        )
        .bind(household_id)
        .bind(employer.account_id)
        .fetch_all(db)
        .await?;
        let (latest,): (Option<Date>,) =
            sqlx::query_as("select max(posted_on) from legs where account_id = $1")
                .bind(employer.account_id)
                .fetch_one(db)
                .await?;
        let deposits: Vec<Deposit<Uuid>> = paid
            .iter()
            .map(|p| Deposit {
                on: p.date,
                amount_minor: p.amount_minor,
                description: p.description.clone(),
                account: employer.account_id,
            })
            .collect();
        let rule = Employer {
            name: employer.name.clone(),
            match_text: employer.match_text.clone(),
            account: employer.account_id,
            min_amount_minor: employer.min_amount_minor,
            amount_minor: employer.amount_minor,
            start: Fortnight::containing(employer.start_month),
        };
        let outcome = report(&rule, &deposits, as_of, latest, labels).map(|r| (r, paid));
        found.push(Owing { employer, outcome });
    }
    Ok(found)
}
