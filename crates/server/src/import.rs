//! Reading a bank file for one of the household's accounts, and storing what it adds. The preview and the import
//! both go through here, so what a preview shows is what the import does.
//!
//! The server reads the file with the account's profile and namespace, so every `external_id` is computed here,
//! never taken from the caller. A transaction whose `external_id` the account already has is a duplicate and is
//! skipped, which makes importing the same file twice add nothing.

use std::collections::{HashMap, HashSet};

use axum::body::Bytes;
use coinroost_normalizer::detect::{AccountRule, Detection, detect};
use coinroost_normalizer::{Normalization, Profile, RowProblem, normalize};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use time::{Date, OffsetDateTime};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::auth::Session;
use crate::error::{ApiError, ApiResult};
use crate::validate;

/// The account a file is read for, and how.
pub struct Target {
    pub account_id: Uuid,
    pub profile_id: String,
    pub namespace: String,
    pub currency: String,
}

/// One of the household's accounts that can take imports.
pub async fn target(db: &PgPool, household_id: Uuid, account_id: Uuid) -> ApiResult<Target> {
    let account: Option<(Option<String>, String, String)> = sqlx::query_as(
        "select profile, namespace, currency from accounts where id = $1 and household_id = $2",
    )
    .bind(account_id)
    .bind(household_id)
    .fetch_optional(db)
    .await?;
    let (profile_id, namespace, currency) = account.ok_or(ApiError::NotFound)?;
    let profile_id = profile_id.ok_or_else(|| ApiError::Invalid("the account has no profile".to_owned()))?;
    if validate::minor_digits(&currency) != Some(2) {
        return Err(ApiError::Invalid(format!(
            "imports into {currency} accounts are not supported yet"
        )));
    }
    Ok(Target {
        account_id,
        profile_id,
        namespace,
        currency,
    })
}

/// Reads the file with the target's profile and namespace.
pub async fn read(file: Bytes, target: &Target) -> ApiResult<Normalization> {
    if file.is_empty() {
        return Err(ApiError::Invalid("the file is empty".to_owned()));
    }
    let profile = Profile::find(&target.profile_id)
        .ok_or_else(|| ApiError::Invalid(format!("no profile {}", target.profile_id)))?;
    let namespace = target.namespace.clone();
    tokio::task::spawn_blocking(move || normalize(&file, &profile, &namespace))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Invalid(e.to_string()))
}

/// Which of the household's accounts a file belongs to, read from the file itself.
pub enum Detected {
    Account(Uuid),
    /// The file fits several accounts; the person chooses.
    Ambiguous(Vec<Uuid>),
    NotFound,
}

pub async fn find_account(
    db: &PgPool,
    household_id: Uuid,
    file: Bytes,
    file_name: String,
) -> ApiResult<Detected> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(Uuid, String, String, Vec<String>, Vec<String>, Option<String>, Option<String>)> =
        sqlx::query_as(
            "select id, namespace, profile, detect_markers, detect_card_prefixes, detect_label, detect_number_hash
             from accounts where household_id = $1 and profile is not null",
        )
        .bind(household_id)
        .fetch_all(db)
        .await?;
    let ids: HashMap<String, Uuid> = rows.iter().map(|r| (r.1.clone(), r.0)).collect();
    let rules: Vec<AccountRule> = rows
        .into_iter()
        .map(
            |(_, namespace, profile, markers, card_prefixes, label, number_hash)| AccountRule {
                namespace,
                profile,
                markers,
                card_prefixes,
                label,
                number_hash,
            },
        )
        .collect();
    let detection =
        tokio::task::spawn_blocking(move || detect(&file, &file_name, &Profile::builtin(), &rules))
            .await
            .map_err(|e| ApiError::Internal(e.to_string()))?;
    let id = |namespace: &String| ids.get(namespace).copied();
    Ok(match detection {
        Detection::Found(namespace) => id(&namespace).map_or(Detected::NotFound, Detected::Account),
        Detection::Ambiguous(namespaces) => Detected::Ambiguous(namespaces.iter().filter_map(id).collect()),
        Detection::NotFound => Detected::NotFound,
    })
}

/// The `external_id`s of the file that the account already has.
pub async fn known_ids(
    db: &mut PgConnection,
    account_id: Uuid,
    normalization: &Normalization,
) -> ApiResult<HashSet<String>> {
    let ids: Vec<&str> = normalization
        .transactions
        .iter()
        .map(|t| t.external_id.as_str())
        .collect();
    let known: Vec<(String,)> =
        sqlx::query_as("select external_id from legs where account_id = $1 and external_id = any($2)")
            .bind(account_id)
            .bind(&ids)
            .fetch_all(db)
            .await?;
    Ok(known.into_iter().map(|(id,)| id).collect())
}

#[derive(Serialize, ToSchema)]
pub struct RowFailure {
    /// Row number in the file, 1 being the header.
    row: usize,
    reason: String,
}

/// The rows that could not be read, and why.
pub fn failures(normalization: &Normalization) -> Vec<RowFailure> {
    normalization
        .errors
        .iter()
        .map(|e| RowFailure {
            row: e.row,
            reason: match &e.problem {
                RowProblem::UnreadableDate(text) => format!("unreadable date: {text}"),
                RowProblem::UnreadableAmount => "no amount that can be read".to_owned(),
            },
        })
        .collect()
}

pub fn parse_date(text: &str) -> ApiResult<Date> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    Date::parse(text, format).map_err(|e| ApiError::Internal(format!("normalizer date {text}: {e}")))
}

/// The first and last date among the rows read, duplicates included.
pub fn date_range(normalization: &Normalization) -> ApiResult<Option<(Date, Date)>> {
    let mut range: Option<(Date, Date)> = None;
    for transaction in &normalization.transactions {
        let date = parse_date(&transaction.date)?;
        range = Some(range.map_or((date, date), |(first, last)| (first.min(date), last.max(date))));
    }
    Ok(range)
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct ImportBatch {
    id: Uuid,
    account_id: Uuid,
    file_name: String,
    /// SHA-256 of the file, hexadecimal.
    #[sqlx(try_from = "Vec<u8>")]
    file_sha256: Hex,
    profile: String,
    /// Data rows in the file, the header excluded.
    rows_read: i32,
    added: i32,
    duplicates: i32,
    errors: i32,
    /// The first and last date among the rows read, duplicates included; empty when no row could be read.
    first_date: Option<Date>,
    last_date: Option<Date>,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
#[serde(transparent)]
#[schema(value_type = String)]
pub struct Hex(String);

impl From<Vec<u8>> for Hex {
    fn from(bytes: Vec<u8>) -> Hex {
        use std::fmt::Write;
        Hex(bytes.iter().fold(String::with_capacity(64), |mut hex, b| {
            let _ = write!(hex, "{b:02x}");
            hex
        }))
    }
}

/// SHA-256 of a file, hexadecimal.
pub fn file_hash(file: &[u8]) -> String {
    Hex::from(Sha256::digest(file).to_vec()).0
}

macro_rules! batch_columns {
    () => {
        "id, account_id, file_name, file_sha256, profile, rows_read, added, duplicates, errors, first_date,
         last_date, created_at"
    };
}

/// Adds the transactions the account does not have yet, and records the import.
pub async fn store(
    db: &PgPool,
    session: Session,
    target: &Target,
    file_name: &str,
    file: &[u8],
    normalization: &Normalization,
) -> ApiResult<ImportBatch> {
    let range = date_range(normalization)?;
    let mut tx = db.begin().await?;
    // One import at a time per account, so two uploads of one file cannot both add it.
    sqlx::query("select id from accounts where id = $1 for update")
        .bind(target.account_id)
        .execute(&mut *tx)
        .await?;
    let known = known_ids(&mut tx, target.account_id, normalization).await?;
    let new: Vec<_> = normalization
        .transactions
        .iter()
        .filter(|t| !known.contains(&t.external_id))
        .collect();
    let count =
        |n: usize| i32::try_from(n).map_err(|_| ApiError::Invalid("the file has too many rows".to_owned()));
    let batch: ImportBatch = sqlx::query_as(concat!(
        "insert into import_batches (household_id, account_id, file_name, file_sha256, profile, rows_read, added,
                                     duplicates, errors, first_date, last_date, created_by)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12) returning ",
        batch_columns!()
    ))
    .bind(session.household_id)
    .bind(target.account_id)
    .bind(file_name)
    .bind(Sha256::digest(file).to_vec())
    .bind(&target.profile_id)
    .bind(count(normalization.summary.rows)?)
    .bind(count(new.len())?)
    .bind(count(normalization.transactions.len() - new.len())?)
    .bind(count(normalization.errors.len())?)
    .bind(range.map(|r| r.0))
    .bind(range.map(|r| r.1))
    .bind(session.user_id)
    .fetch_one(&mut *tx)
    .await?;
    for transaction in new {
        let date = parse_date(&transaction.date)?;
        let (id,): (Uuid,) = sqlx::query_as(
            "insert into transactions (household_id, date, description) values ($1, $2, $3) returning id",
        )
        .bind(session.household_id)
        .bind(date)
        .bind(&transaction.description)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "insert into legs (transaction_id, household_id, account_id, amount_minor, currency, posted_on,
                               statement_text, external_id, import_batch_id)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(id)
        .bind(session.household_id)
        .bind(target.account_id)
        .bind(transaction.amount.minor_units())
        .bind(&target.currency)
        .bind(date)
        .bind(&transaction.description)
        .bind(&transaction.external_id)
        .bind(batch.id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(batch)
}

/// The household's imports, newest first.
pub async fn batches(db: &PgPool, household_id: Uuid) -> ApiResult<Vec<ImportBatch>> {
    Ok(sqlx::query_as(concat!(
        "select ",
        batch_columns!(),
        " from import_batches where household_id = $1 order by created_at desc, id"
    ))
    .bind(household_id)
    .fetch_all(db)
    .await?)
}
