//! Who is calling: users, their login and their sessions.
//!
//! - Passwords are hashed with Argon2id at OWASP's recommended cost (19 MiB, 2 passes, 1 lane).
//! - A login needs the password and a current TOTP code (RFC 6238: SHA-1, 6 digits, 30-second steps, one step of
//!   clock drift either way). A code is accepted once: the step it matched must be later than the last one used.
//! - Five failed logins in a row lock the user for 15 minutes. Every failure, whatever its cause (unknown email,
//!   wrong password, wrong code, locked), gets the same answer, and an unknown email still costs a hash, so the
//!   answer does not tell which emails exist.
//! - A session is 256 random bits, sent as an `HttpOnly`, `Secure`, `SameSite=Strict` cookie or as a bearer token;
//!   only its SHA-256 is stored. Sessions last 7 days.
//! - An API token (`crt_…`, see [`crate::tokens`]) is accepted as a bearer only, never from a cookie; a `read`
//!   token may only read (GET and HEAD), and any other method answers 403.

use std::sync::LazyLock;
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum::http::{HeaderMap, Method, header};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};
use sqlx::{PgExecutor, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::AppState;
use crate::error::{ApiError, ApiResult};
use crate::tokens;

pub const SESSION_COOKIE: &str = "coinroost_session";
pub const SESSION_DAYS: i64 = 7;
const MAX_FAILED_LOGINS: i32 = 5;
const LOCK_MINUTES: i64 = 15;
const MIN_PASSWORD_CHARS: usize = 12;
const TOTP_STEP: u64 = 30;

/// The caller of a request: a user with a live session, and their household. Every query a handler runs is
/// scoped to `household_id`.
#[derive(Clone, Copy, Debug)]
pub struct Session {
    pub user_id: Uuid,
    pub household_id: Uuid,
}

impl<S> FromRequestParts<S> for Session
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Session, ApiError> {
        let state = AppState::from_ref(state);
        if let Some(token) = bearer(&parts.headers).filter(|t| t.starts_with(tokens::PREFIX)) {
            return api_token(&state, &token, &parts.method).await;
        }
        let token = session_token(&parts.headers).ok_or(ApiError::Unauthorized)?;
        let row: Option<(Uuid, Uuid)> = sqlx::query_as(
            "select u.id, u.household_id from sessions s join users u on u.id = s.user_id
             where s.token_hash = $1 and s.expires_at > now()",
        )
        .bind(token_hash(&token))
        .fetch_optional(&state.db)
        .await?;
        let (user_id, household_id) = row.ok_or(ApiError::Unauthorized)?;
        Ok(Session {
            user_id,
            household_id,
        })
    }
}

/// The caller of an API token: its user, if the token is live, and only for reading when it is a read token.
async fn api_token(state: &AppState, token: &str, method: &Method) -> Result<Session, ApiError> {
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        "with used as (update api_tokens set last_used_at = now() where token_hash = $1 and revoked_at is null
                       returning user_id, scope)
         select u.id, u.household_id, used.scope from used join users u on u.id = used.user_id",
    )
    .bind(token_hash(token))
    .fetch_optional(&state.db)
    .await?;
    let (user_id, household_id, scope) = row.ok_or(ApiError::Unauthorized)?;
    if scope != "write" && !matches!(*method, Method::GET | Method::HEAD) {
        return Err(ApiError::Forbidden(
            "this token may only read: ask for a write token".to_owned(),
        ));
    }
    Ok(Session {
        user_id,
        household_id,
    })
}

/// The token of `Authorization: Bearer …`, if any.
pub fn bearer(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
}

/// The token from `Authorization: Bearer …`, else from the session cookie.
pub fn session_token(headers: &HeaderMap) -> Option<String> {
    if let Some(token) = bearer(headers) {
        return Some(token);
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.to_owned())
}

pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

pub fn hash_password(password: &str) -> ApiResult<String> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(ApiError::Invalid(format!(
            "a password needs at least {MIN_PASSWORD_CHARS} characters"
        )));
    }
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// A household with nobody in it yet.
pub async fn create_household(db: impl PgExecutor<'_>, name: &str) -> ApiResult<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as("insert into households (name) values ($1) returning id")
        .bind(name.trim())
        .fetch_one(db)
        .await?;
    Ok(id)
}

/// Adds a user to a household and returns their id and the TOTP secret to enroll in an authenticator app.
pub async fn create_user(
    db: impl PgExecutor<'_>,
    household_id: Uuid,
    email: &str,
    password: &str,
) -> ApiResult<(Uuid, Vec<u8>)> {
    let secret = new_secret();
    let enrolled = Enrolled {
        secret: &secret,
        last_step: 0,
    };
    let id = insert_user(db, household_id, email, &hash_password(password)?, enrolled).await?;
    Ok((id, secret))
}

/// A secret for an authenticator app: 160 random bits, the size RFC 4226 recommends.
pub fn new_secret() -> Vec<u8> {
    rand::random::<[u8; 20]>().to_vec()
}

/// The authenticator a new user starts with: its secret and the last step a code was accepted for.
pub struct Enrolled<'a> {
    pub secret: &'a [u8],
    pub last_step: i64,
}

/// The advisory lock every user insert takes, held to the end of its transaction, so a first-account sign-up can
/// check that no user exists and add one with nothing slipping in between (see [`crate::invites`]).
pub const USERS_LOCK: i64 = 0x636f_696e_726f_6f73;

/// Stores a user whose password is already hashed ([`hash_password`]), under [`USERS_LOCK`].
pub async fn insert_user(
    db: impl PgExecutor<'_>,
    household_id: Uuid,
    email: &str,
    password_hash: &str,
    enrolled: Enrolled<'_>,
) -> ApiResult<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "insert into users (household_id, email, password_hash, totp_secret, totp_last_step)
         select $1, $2, $3, $4, $5 from (select pg_advisory_xact_lock($6)) as serialized
         returning id",
    )
    .bind(household_id)
    .bind(email.trim().to_lowercase())
    .bind(password_hash)
    .bind(enrolled.secret)
    .bind(enrolled.last_step)
    .bind(USERS_LOCK)
    .fetch_one(db)
    .await?;
    Ok(id)
}

/// A user added from the console, and what their authenticator app needs.
pub struct Enrollment {
    pub household_id: Uuid,
    pub user_id: Uuid,
    /// The `otpauth://` URI an authenticator app enrolls from.
    pub uri: String,
    /// The same secret in base32, for apps where it is typed in.
    pub secret_base32: String,
}

/// Adds a user to a household, or to a new one when none is given; both or neither are created.
pub async fn enroll(
    db: &PgPool,
    email: &str,
    password: &str,
    household: Option<Uuid>,
) -> ApiResult<Enrollment> {
    let mut tx = db.begin().await?;
    let household_id = match household {
        Some(id) => id,
        None => create_household(&mut *tx, "Household").await?,
    };
    let (user_id, secret) = create_user(&mut *tx, household_id, email, password).await?;
    tx.commit().await?;
    let account = email.trim().to_lowercase();
    Ok(Enrollment {
        household_id,
        user_id,
        uri: enrollment_uri(&account, &secret)?,
        secret_base32: secret_base32(&secret)?,
    })
}

/// The secret in base32, for apps where it is typed in.
pub fn secret_base32(secret: &[u8]) -> ApiResult<String> {
    Ok(totp(secret, "user")?.secret().to_base32())
}

/// The time step a code matches now (one step of drift either way), or `None` when it does not match. The code may
/// come as people copy it, with spaces or dashes ("123 456", "123-456"); anything but six digits after that is wrong.
pub fn code_step(secret: &[u8], code: &str) -> ApiResult<Option<i64>> {
    let digits: String = code
        .chars()
        .filter(|c| !c.is_whitespace() && !DASHES.contains(c))
        .collect();
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Ok(None);
    }
    Ok(totp(secret, "user")?
        .check(&digits, now_seconds())
        .and_then(|step| i64::try_from(step).ok()))
}

/// The hyphen-minus and the Unicode hyphens and dashes a copied code may carry between its groups.
const DASHES: [char; 6] = ['-', '\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}'];

/// The `otpauth://` URI an authenticator app enrolls from.
pub fn enrollment_uri(email: &str, secret: &[u8]) -> ApiResult<String> {
    totp(secret, email)?
        .to_url()
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn totp(secret: &[u8], account: &str) -> ApiResult<totp_rs::Totp> {
    totp_rs::Builder::new()
        .with_algorithm(totp_rs::Algorithm::SHA1)
        .with_digits(6)
        .with_skew(1)
        .with_step_duration(TOTP_STEP)
        .with_secret(secret)
        .with_issuer(Some("Coinroost"))
        .with_account_name(account)
        .build()
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// The current code for a secret, as an authenticator app shows it (tests use it to log in).
pub fn current_code(secret: &[u8]) -> ApiResult<String> {
    Ok(totp(secret, "user")?.generate(now_seconds()).to_string())
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// A new session for the user, or `Unauthorized` for any failed check.
pub async fn login(
    db: &PgPool,
    email: &str,
    password: &str,
    code: &str,
) -> ApiResult<(String, OffsetDateTime)> {
    let row: Option<(Uuid, String, Vec<u8>, i64, bool)> = sqlx::query_as(
        "select id, password_hash, totp_secret, totp_last_step, coalesce(locked_until > now(), false)
         from users where email = $1",
    )
    .bind(email.trim().to_lowercase())
    .fetch_optional(db)
    .await?;
    let Some((user_id, password_hash, secret, last_step, locked)) = row else {
        // Same cost as a real check, so timing does not reveal unknown emails.
        let _ = Argon2::default().verify_password(password.as_bytes(), DUMMY_HASH.as_str());
        return Err(ApiError::Unauthorized);
    };
    if locked {
        return Err(ApiError::Unauthorized);
    }
    let password_ok = Argon2::default()
        .verify_password(password.as_bytes(), password_hash.as_str())
        .is_ok();
    let step = code_step(&secret, code)?;
    let code_ok = step.is_some_and(|step| step > last_step);
    if !(password_ok && code_ok) {
        sqlx::query(
            "update users set failed_logins = failed_logins + 1,
             locked_until = case when failed_logins + 1 >= $2 then now() + make_interval(mins => $3) end
             where id = $1",
        )
        .bind(user_id)
        .bind(MAX_FAILED_LOGINS)
        .bind(i32::try_from(LOCK_MINUTES).unwrap_or(15))
        .execute(db)
        .await?;
        return Err(ApiError::Unauthorized);
    }
    // Claims the step; a concurrent login with the same code finds it taken.
    let claimed = sqlx::query(
        "update users set totp_last_step = $2, failed_logins = 0, locked_until = null
         where id = $1 and totp_last_step < $2",
    )
    .bind(user_id)
    .bind(step)
    .execute(db)
    .await?;
    if claimed.rows_affected() == 0 {
        return Err(ApiError::Unauthorized);
    }
    new_session(db, user_id).await
}

/// Opens a session for the user and returns its token, which is stored only as its hash.
pub async fn new_session(db: impl PgExecutor<'_>, user_id: Uuid) -> ApiResult<(String, OffsetDateTime)> {
    let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>());
    let expires_at = OffsetDateTime::now_utc() + time::Duration::days(SESSION_DAYS);
    sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, $3)")
        .bind(token_hash(&token))
        .bind(user_id)
        .bind(expires_at)
        .execute(db)
        .await?;
    Ok((token, expires_at))
}

/// An Argon2id hash of a random password, checked against when the email is unknown.
static DUMMY_HASH: LazyLock<String> = LazyLock::new(|| {
    let password = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 16]>());
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .unwrap_or_default()
});

/// The `Set-Cookie` value for a session.
pub fn session_cookie(token: &str) -> String {
    let max_age = SESSION_DAYS * 24 * 3600;
    format!("{SESSION_COOKIE}={token}; Path=/; Max-Age={max_age}; HttpOnly; Secure; SameSite=Strict")
}

/// The `Set-Cookie` value that clears the session cookie.
pub fn cleared_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; Secure; SameSite=Strict")
}
