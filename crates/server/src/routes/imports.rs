//! Bank files imported into an account, and the preview that shows what an import would do before it is made.
//! Both read the file the same way (`crate::import`); the preview writes nothing.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use time::Date;
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use uuid::Uuid;

use crate::AppState;
use crate::auth::Session;
use crate::error::{ApiResult, Problem};
use crate::import::{self, Detected, ImportBatch, RowFailure};
use crate::validate;

/// Statements are a few hundred kilobytes; this leaves room for a year of a busy account.
const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;

pub fn router() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(import_file))
        .routes(routes!(preview_file))
        .layer(DefaultBodyLimit::max(MAX_FILE_BYTES))
        .routes(routes!(list_imports))
}

/// A bank export, byte for byte as the bank gave it.
#[derive(ToSchema)]
#[schema(value_type = String, format = Binary)]
#[expect(dead_code, reason = "it only describes the request body in the contract")]
pub struct BankFile(Vec<u8>);

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct FileName {
    /// The file's name, as the bank gave it.
    file_name: String,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PreviewQuery {
    /// The file's name, as the bank gave it.
    file_name: String,
    /// The account to read the file for; when absent, the account is found from the file.
    account_id: Option<Uuid>,
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

/// How the preview chose the account.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountMatch {
    /// The caller named it.
    Chosen,
    /// The file fits exactly one account.
    Found,
    /// The file fits several accounts: see `candidates`.
    Ambiguous,
    /// The file fits none of the household's accounts.
    NotFound,
}

#[derive(Serialize, ToSchema)]
pub struct PreviewRow {
    date: Date,
    description: String,
    /// Signed minor units in the account's currency: negative is money leaving the account.
    amount_minor: i64,
    external_id: String,
    /// The account already has it: the import skips it.
    duplicate: bool,
}

/// What importing the file into the account would do.
#[derive(Serialize, ToSchema)]
pub struct Reading {
    /// The transactions read, in file order.
    rows: Vec<PreviewRow>,
    /// Rows that cannot be read; the import leaves them out.
    row_errors: Vec<RowFailure>,
    /// Data rows in the file, the header excluded.
    rows_read: usize,
    new: usize,
    duplicates: usize,
    errors: usize,
    /// The first and last date among the rows read, duplicates included.
    first_date: Option<Date>,
    last_date: Option<Date>,
}

#[derive(Serialize, ToSchema)]
pub struct ImportPreview {
    file_name: String,
    /// SHA-256 of the file, hexadecimal.
    file_sha256: String,
    detection: AccountMatch,
    account_id: Option<Uuid>,
    /// The accounts the file fits, when it fits several.
    candidates: Vec<Uuid>,
    /// Empty until the account is known.
    reading: Option<Reading>,
}

/// Imports a bank file into an account, with the account's profile. Transactions already in the account are
/// skipped; rows that cannot be read are reported and left out.
#[utoipa::path(post, path = "/api/v1/accounts/{account_id}/imports", tag = "imports",
    params(("account_id" = Uuid, Path, description = "The account the file belongs to"), FileName),
    request_body(content = BankFile, content_type = "application/octet-stream", description = "The file as exported"),
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
    let target = import::target(&state.db, session.household_id, account_id).await?;
    let normalization = import::read(file.clone(), &target).await?;
    let batch = import::store(&state.db, session, &target, &file_name, &file, &normalization).await?;
    let row_errors = import::failures(&normalization);
    Ok((StatusCode::CREATED, Json(ImportReport { batch, row_errors })))
}

/// Shows what importing a bank file would do, without importing it: the account it belongs to (found from the
/// file unless named), every transaction read and whether the account already has it, and the rows that cannot
/// be read.
#[utoipa::path(post, path = "/api/v1/imports/preview", tag = "imports", params(PreviewQuery),
    request_body(content = BankFile, content_type = "application/octet-stream", description = "The file as exported"),
    responses(
        (status = OK, description = "What the import would do", body = ImportPreview),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = NOT_FOUND, description = "No such account", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "The file cannot be read with the account's profile", body = Problem)))]
async fn preview_file(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<PreviewQuery>,
    file: Bytes,
) -> ApiResult<Json<ImportPreview>> {
    let file_name = validate::text("file_name", &query.file_name, 255)?;
    let (detection, account_id, candidates) = match query.account_id {
        Some(id) => (AccountMatch::Chosen, Some(id), Vec::new()),
        None => match import::find_account(&state.db, session.household_id, file.clone(), file_name.clone())
            .await?
        {
            Detected::Account(id) => (AccountMatch::Found, Some(id), Vec::new()),
            Detected::Ambiguous(ids) => (AccountMatch::Ambiguous, None, ids),
            Detected::NotFound => (AccountMatch::NotFound, None, Vec::new()),
        },
    };
    let reading = match account_id {
        Some(id) => Some(read(&state, session, id, file.clone()).await?),
        None => None,
    };
    Ok(Json(ImportPreview {
        file_name,
        file_sha256: import::file_hash(&file),
        detection,
        account_id,
        candidates,
        reading,
    }))
}

async fn read(state: &AppState, session: Session, account_id: Uuid, file: Bytes) -> ApiResult<Reading> {
    let target = import::target(&state.db, session.household_id, account_id).await?;
    let normalization = import::read(file, &target).await?;
    let mut db = state.db.acquire().await?;
    let known = import::known_ids(&mut db, account_id, &normalization).await?;
    let rows = normalization
        .transactions
        .iter()
        .map(|t| {
            Ok(PreviewRow {
                date: import::parse_date(&t.date)?,
                description: t.description.clone(),
                amount_minor: t.amount.minor_units(),
                external_id: t.external_id.clone(),
                duplicate: known.contains(&t.external_id),
            })
        })
        .collect::<ApiResult<Vec<_>>>()?;
    let duplicates = rows.iter().filter(|r| r.duplicate).count();
    let range = import::date_range(&normalization)?;
    Ok(Reading {
        new: rows.len() - duplicates,
        duplicates,
        rows,
        row_errors: import::failures(&normalization),
        rows_read: normalization.summary.rows,
        errors: normalization.errors.len(),
        first_date: range.map(|r| r.0),
        last_date: range.map(|r| r.1),
    })
}

/// The household's imports, newest first.
#[utoipa::path(get, path = "/api/v1/imports", tag = "imports",
    responses(
        (status = OK, description = "The imports", body = ImportBatches),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_imports(State(state): State<AppState>, session: Session) -> ApiResult<Json<ImportBatches>> {
    let items = import::batches(&state.db, session.household_id).await?;
    Ok(Json(ImportBatches { items }))
}
