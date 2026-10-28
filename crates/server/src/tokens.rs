//! Long-lived API tokens: added and revoked from the console, used as `Authorization: Bearer crt_…`.
//!
//! A token is `crt_` and 256 random bits in base64url; the prefix tells it from a session token at a glance and
//! lets secret scanners find a leaked one. Only its SHA-256 is stored. A `read` token may only read: the API
//! refuses any other method with 403 (see [`crate::auth::Session`]).

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::token_hash;

pub const PREFIX: &str = "crt_";

/// What a token may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Read,
    Write,
}

impl Scope {
    fn as_str(self) -> &'static str {
        match self {
            Scope::Read => "read",
            Scope::Write => "write",
        }
    }
}

async fn user_id(db: &PgPool, email: &str) -> Result<Uuid, String> {
    let row: Option<(Uuid,)> = sqlx::query_as("select id from users where email = $1")
        .bind(email.trim().to_lowercase())
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?;
    row.map(|(id,)| id).ok_or_else(|| format!("no user {email:?}"))
}

/// A new token for the user, named so it can be told apart and revoked; the token is returned once and never
/// stored.
pub async fn add(db: &PgPool, email: &str, name: &str, scope: Scope) -> Result<String, String> {
    let user = user_id(db, email).await?;
    let bytes: [u8; 32] = rand::random();
    let token = format!("{PREFIX}{}", URL_SAFE_NO_PAD.encode(bytes));
    sqlx::query("insert into api_tokens (user_id, name, token_hash, scope) values ($1, $2, $3, $4)")
        .bind(user)
        .bind(name.trim())
        .bind(token_hash(&token))
        .bind(scope.as_str())
        .execute(db)
        .await
        .map_err(|e| {
            match e
                .as_database_error()
                .and_then(sqlx::error::DatabaseError::code)
                .as_deref()
            {
                Some("23505") => format!("{email} already has a live token named {name:?}"),
                Some("23514") => "a token name has 1 to 100 characters".to_owned(),
                _ => e.to_string(),
            }
        })?;
    Ok(token)
}

/// Revokes the user's live token of that name; false when there is none.
pub async fn revoke(db: &PgPool, email: &str, name: &str) -> Result<bool, String> {
    let user = user_id(db, email).await?;
    let done = sqlx::query(
        "update api_tokens set revoked_at = now() where user_id = $1 and name = $2 and revoked_at is null",
    )
    .bind(user)
    .bind(name.trim())
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(done.rows_affected() == 1)
}
