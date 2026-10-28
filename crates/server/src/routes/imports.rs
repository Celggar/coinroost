//! Bank files imported into an account. The server reads the file with the account's profile and namespace, so
//! every `external_id` is computed here, never taken from the caller; a transaction whose `external_id` the
//! account already has is a duplicate and is skipped, which makes importing the same file twice add nothing.

use std::collections::HashSet;

use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use coinroost_normalizer::{Normalization, Profile, RowProblem, normalize};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::{Date, OffsetDateTime};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiError, ApiResult, Problem};
use crate::validate;

/// Statements are a few hundred kilobytes; this leaves room for a year of a busy account.
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(import_file))
        .layer(DefaultBodyLimit::max(MAX_FILE_BYTES))
        .routes(routes!(list_imports))
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FileName {
    /// The file's name, as the bank gave it.
    file_name: String,
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

/// The account a file goes into, and how it is read.
struct Target {
    account_id: Uuid,
    profile_id: String,
    currency: String,
    file_name: String,
}

#[derive(Serialize, ToSchema)]
pub struct RowFailure {
    /// Row number in the file, 1 being the header.
    row: usize,
    reason: String,
}

#[derive(Serialize, ToSchema)]
pub struct ImportReport {
    batch: ImportBatch,
    /// Rows that could not be read; they were not imported.
    row_errors: Vec<RowFailure>,
}

#[derive(Serialize, ToSchema)]
pub struct ImportBatches {
    items: Vec<ImportBatch>,
}

macro_rules! columns {
    () => {
        "id, account_id, file_name, file_sha256, profile, rows_read, added, duplicates, errors, created_at"
    };
}

/// Imports a bank file into an account, with the account's profile. Transactions already in the account are
/// skipped; rows that cannot be read are reported and left out.
#[utoipa::path(post, path = "/api/v1/accounts/{account_id}/imports", tag = "imports",
    params(("account_id" = Uuid, Path, description = "The account the file belongs to"), FileName),
    request_body(content = Vec<u8>, content_type = "application/octet-stream", description = "The file as exported"),
    responses(
        (status = CREATED, description = "What was added and what was skipped", body = ImportReport),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such account", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The file cannot be read with the account's profile", body = Problem)))]
async fn import_file(
    State(state): State<AppState>,
    session: Session,
    Path(account_id): Path<Uuid>,
    Query(query): Query<FileName>,
    file: Bytes,
) -> ApiResult<(StatusCode, Json<ImportReport>)> {
    let file_name = validate::text("file_name", &query.file_name, 255)?;
    let account: Option<(Option<String>, String, String)> = sqlx::query_as(
        "select profile, namespace, currency from accounts where id = $1 and household_id = $2",
    )
    .bind(account_id)
    .bind(session.household_id)
    .fetch_optional(&state.db)
    .await?;
    let (profile_id, namespace, currency) = account.ok_or(ApiError::NotFound)?;
    let profile_id = profile_id.ok_or_else(|| ApiError::Invalid("the account has no profile".to_owned()))?;
    let profile =
        Profile::find(&profile_id).ok_or_else(|| ApiError::Invalid(format!("no profile {profile_id}")))?;
    if validate::minor_digits(&currency) != Some(2) {
        return Err(ApiError::Invalid(format!(
            "imports into {currency} accounts are not supported yet"
        )));
    }
    if file.is_empty() {
        return Err(ApiError::Invalid("the file is empty".to_owned()));
    }
    let bytes = file.clone();
    let normalization = tokio::task::spawn_blocking(move || normalize(&bytes, &profile, &namespace))
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .map_err(|e| ApiError::Invalid(e.to_string()))?;
    let target = Target {
        account_id,
        profile_id,
        currency,
        file_name,
    };
    let batch = store(&state, session, &target, &file, &normalization).await?;
    let row_errors = normalization
        .errors
        .iter()
        .map(|e| RowFailure {
            row: e.row,
            reason: match &e.problem {
                RowProblem::UnreadableDate(text) => format!("unreadable date: {text}"),
                RowProblem::UnreadableAmount => "no amount that can be read".to_owned(),
            },
        })
        .collect();
    Ok((StatusCode::CREATED, Json(ImportReport { batch, row_errors })))
}

/// Adds the transactions the account does not have yet, and records the import.
async fn store(
    state: &AppState,
    session: Session,
    target: &Target,
    file: &[u8],
    normalization: &Normalization,
) -> ApiResult<ImportBatch> {
    let mut tx = state.db.begin().await?;
    // One import at a time per account, so two uploads of one file cannot both add it.
    sqlx::query("select id from accounts where id = $1 for update")
        .bind(target.account_id)
        .execute(&mut *tx)
        .await?;
    let ids: Vec<&str> = normalization
        .transactions
        .iter()
        .map(|t| t.external_id.as_str())
        .collect();
    let known: Vec<(String,)> =
        sqlx::query_as("select external_id from legs where account_id = $1 and external_id = any($2)")
            .bind(target.account_id)
            .bind(&ids)
            .fetch_all(&mut *tx)
            .await?;
    let known: HashSet<String> = known.into_iter().map(|(id,)| id).collect();
    let new: Vec<_> = normalization
        .transactions
        .iter()
        .filter(|t| !known.contains(&t.external_id))
        .collect();
    let count =
        |n: usize| i32::try_from(n).map_err(|_| ApiError::Invalid("the file has too many rows".to_owned()));
    let batch: ImportBatch = sqlx::query_as(concat!(
        "insert into import_batches (household_id, account_id, file_name, file_sha256, profile, rows_read, added,
                                     duplicates, errors, created_by)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) returning ",
        columns!()
    ))
    .bind(session.household_id)
    .bind(target.account_id)
    .bind(&target.file_name)
    .bind(Sha256::digest(file).to_vec())
    .bind(&target.profile_id)
    .bind(count(normalization.summary.rows)?)
    .bind(count(new.len())?)
    .bind(count(normalization.transactions.len() - new.len())?)
    .bind(count(normalization.errors.len())?)
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

fn parse_date(text: &str) -> ApiResult<Date> {
    let format = time::macros::format_description!("[year]-[month]-[day]");
    Date::parse(text, format).map_err(|e| ApiError::Internal(format!("normalizer date {text}: {e}")))
}

/// The household's imports, newest first.
#[utoipa::path(get, path = "/api/v1/imports", tag = "imports",
    responses(
        (status = OK, description = "The imports", body = ImportBatches),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_imports(State(state): State<AppState>, session: Session) -> ApiResult<Json<ImportBatches>> {
    let items = sqlx::query_as(concat!(
        "select ",
        columns!(),
        " from import_batches where household_id = $1 order by created_at desc, id"
    ))
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(ImportBatches { items }))
}
