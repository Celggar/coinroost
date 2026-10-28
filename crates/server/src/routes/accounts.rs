//! The household's accounts. An account's `namespace` goes into the `external_id` of everything imported into
//! it, and its `profile` says how its bank exports are read.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use coinroost_normalizer::{FileFormat, Profile};
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
    OpenApiRouter::new()
        .routes(routes!(list_accounts, create_account))
        .routes(routes!(list_profiles))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AccountKind {
    Checking,
    Savings,
    CreditCard,
    Cash,
    Loan,
    Investment,
}

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewAccount {
    name: String,
    kind: AccountKind,
    /// ISO 4217 code.
    currency: String,
    /// Lower case letters, digits, `_` and `-`; cannot change once the account has transactions.
    namespace: String,
    /// Id of the bank profile its exports are read with (`GET /api/v1/profiles` lists them).
    profile: Option<String>,
    #[serde(default)]
    detection: Detection,
}

/// How the account's exports are recognized when several files are dropped at once. A file belongs to the
/// account when its header has every detection text of the profile and every marker, and, when a label or a
/// number hash is set, at least one of them. Cards that share a format are told apart by card prefix.
#[derive(Default, Deserialize, Serialize, ToSchema, sqlx::FromRow)]
#[serde(deny_unknown_fields)]
pub struct Detection {
    /// Texts the header must also contain, such as the account name as the bank prints it.
    #[serde(default)]
    #[sqlx(rename = "detect_markers")]
    markers: Vec<String>,
    /// Card network prefixes (ISO/IEC 7812), such as `4` for Visa or `34` and `37` for American Express.
    #[serde(default)]
    #[sqlx(rename = "detect_card_prefixes")]
    card_prefixes: Vec<String>,
    /// The account type as the bank prints it in the header.
    #[sqlx(rename = "detect_label")]
    label: Option<String>,
    /// First 16 hex digits of the SHA-256 of the account number printed in the header.
    #[sqlx(rename = "detect_number_hash")]
    number_hash: Option<String>,
}

impl Detection {
    fn validated(self) -> ApiResult<Detection> {
        let markers = self
            .markers
            .iter()
            .map(|m| validate::text("detection.markers", m, 200))
            .collect::<ApiResult<Vec<_>>>()?;
        let digits = |p: &String| (1..=6).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit());
        if !self.card_prefixes.iter().all(digits) {
            return Err(ApiError::Invalid(
                "detection.card_prefixes are 1 to 6 digits each".to_owned(),
            ));
        }
        let label = self
            .label
            .map(|l| validate::text("detection.label", &l, 200))
            .transpose()?;
        let hex = |h: &String| h.len() == 16 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
        if self.number_hash.as_ref().is_some_and(|h| !hex(h)) {
            return Err(ApiError::Invalid(
                "detection.number_hash is 16 lower-case hex digits".to_owned(),
            ));
        }
        Ok(Detection {
            markers,
            card_prefixes: self.card_prefixes,
            label,
            number_hash: self.number_hash,
        })
    }
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Account {
    id: Uuid,
    name: String,
    kind: AccountKind,
    currency: String,
    namespace: String,
    profile: Option<String>,
    #[sqlx(flatten)]
    detection: Detection,
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
}

#[derive(Serialize, ToSchema)]
pub struct Accounts {
    items: Vec<Account>,
}

macro_rules! columns {
    () => {
        "id, name, kind, currency, namespace, profile, detect_markers, detect_card_prefixes, detect_label,
         detect_number_hash, created_at"
    };
}

/// The household's accounts, by name.
#[utoipa::path(get, path = "/api/v1/accounts", tag = "accounts",
    responses(
        (status = OK, description = "The accounts", body = Accounts),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_accounts(State(state): State<AppState>, session: Session) -> ApiResult<Json<Accounts>> {
    let items = sqlx::query_as(concat!(
        "select ",
        columns!(),
        " from accounts where household_id = $1 order by name"
    ))
    .bind(session.household_id)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(Accounts { items }))
}

/// Opens an account. Its currency must be the profile's, when it has one.
#[utoipa::path(post, path = "/api/v1/accounts", tag = "accounts", request_body = NewAccount,
    responses(
        (status = CREATED, description = "The new account", body = Account),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem),
        (status = CONFLICT, description = "The name or namespace is taken", body = Problem),
        (status = UNPROCESSABLE_ENTITY, description = "A value is not valid", body = Problem)))]
async fn create_account(
    State(state): State<AppState>,
    session: Session,
    Json(body): Json<NewAccount>,
) -> ApiResult<(StatusCode, Json<Account>)> {
    let name = validate::text("name", &body.name, 200)?;
    let currency = validate::currency(&body.currency)?;
    let namespace = validate::namespace(&body.namespace)?;
    let detection = body.detection.validated()?;
    if let Some(id) = &body.profile {
        let profile = Profile::find(id).ok_or_else(|| ApiError::Invalid(format!("no profile {id}")))?;
        if profile.currency != currency {
            return Err(ApiError::Invalid(format!(
                "profile {id} reads {} statements, the account is in {currency}",
                profile.currency
            )));
        }
    }
    let account = sqlx::query_as(concat!(
        "insert into accounts (household_id, name, kind, currency, namespace, profile, detect_markers,
                               detect_card_prefixes, detect_label, detect_number_hash)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) returning ",
        columns!()
    ))
    .bind(session.household_id)
    .bind(name)
    .bind(body.kind)
    .bind(currency)
    .bind(namespace)
    .bind(body.profile)
    .bind(detection.markers)
    .bind(detection.card_prefixes)
    .bind(detection.label)
    .bind(detection.number_hash)
    .fetch_one(&state.db)
    .await?;
    Ok((StatusCode::CREATED, Json(account)))
}

#[derive(Serialize, ToSchema)]
pub struct BankProfile {
    id: String,
    name: String,
    /// `csv`, `xlsx` or `pdf`.
    format: &'static str,
    /// ISO 4217 code of the statements it reads.
    currency: String,
}

#[derive(Serialize, ToSchema)]
pub struct BankProfiles {
    items: Vec<BankProfile>,
}

/// The bank export formats the server can read, by name.
#[utoipa::path(get, path = "/api/v1/profiles", tag = "accounts",
    responses(
        (status = OK, description = "The profiles", body = BankProfiles),
        (status = UNAUTHORIZED, description = "No valid session", body = Problem)))]
async fn list_profiles(_session: Session) -> Json<BankProfiles> {
    let mut items: Vec<BankProfile> = Profile::builtin()
        .into_iter()
        .map(|p| BankProfile {
            format: match p.format {
                FileFormat::Csv => "csv",
                FileFormat::Xlsx => "xlsx",
                FileFormat::Pdf => "pdf",
            },
            id: p.id,
            name: p.name,
            currency: p.currency,
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(&b.name));
    Json(BankProfiles { items })
}
