//! Recovery codes, and the new authenticator a recovery leads to.
//!
//! - A user holds a set of [`SET_SIZE`] single-use codes (NIST SP 800-63B-4 look-up secrets). A code is 10 characters
//!   of Crockford's Base32 (50 random bits), shown as `xxxxx-xxxxx`; typing ignores case, spaces and dashes, and reads
//!   `o` as `0` and `i` or `l` as `1`, as Crockford's decoding does. Each is stored only as an Argon2id hash: NIST asks
//!   for a salted, costly hash below 112 bits, and the login lockout limits guessing.
//! - The set is made when a setup link is accepted, and again from the Security page, which first asks for a current
//!   authenticator code (OWASP: re-authenticate before changing a factor). A new set replaces the old one at once.
//! - A login with a recovery code always checks [`SET_SIZE`] hashes, real or dummy, whatever the email, password or
//!   code, so its timing tells nothing. A code is claimed once, even by two logins at the same moment.
//! - Such a login retires the authenticator (whoever has the lost phone can no longer use its codes) and ends every
//!   other session of the user (the lost phone may still hold one). Until a new authenticator is confirmed, the user's
//!   sessions may only set it up ([`crate::auth::AnySession`]).

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use sqlx::{PgConnection, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::auth;
use crate::error::{ApiError, ApiResult};
use crate::invites;
use crate::totp;

pub const SET_SIZE: usize = 10;
const CODE_CHARS: usize = 10;
/// Crockford's Base32 alphabet, lowercase: digits and letters without i, l, o and u.
const ALPHABET: &[u8; 32] = b"0123456789abcdefghjkmnpqrstvwxyz";

/// A new code, as shown: `xxxxx-xxxxx`.
fn new_code() -> String {
    // 256 is a multiple of 32, so the low five bits of a random byte pick a character uniformly.
    let chars: Vec<char> = rand::random::<[u8; CODE_CHARS]>()
        .iter()
        .map(|b| char::from(ALPHABET[usize::from(b & 31)]))
        .collect();
    let (head, tail) = chars.split_at(CODE_CHARS / 2);
    format!("{}-{}", String::from_iter(head), String::from_iter(tail))
}

/// A code as typed, reduced to the ten characters that were hashed, or `None` when it cannot be a code.
pub fn normalize(typed: &str) -> Option<String> {
    let code: String = typed
        .chars()
        .filter(|c| !c.is_whitespace() && !totp::DASHES.contains(c))
        .map(|c| match c.to_ascii_lowercase() {
            'o' => '0',
            'i' | 'l' => '1',
            other => other,
        })
        .collect();
    let valid = code.len() == CODE_CHARS && code.bytes().all(|b| ALPHABET.contains(&b));
    valid.then_some(code)
}

/// A set just made: the codes are shown once, here.
pub struct NewSet {
    pub codes: Vec<String>,
    pub created_at: OffsetDateTime,
}

/// Replaces the user's codes with a new set, inside the caller's transaction.
pub async fn issue(tx: &mut PgConnection, user_id: Uuid) -> ApiResult<NewSet> {
    let codes: Vec<String> = (0..SET_SIZE).map(|_| new_code()).collect();
    let hashes = codes
        .iter()
        .map(|code| {
            let typed = normalize(code).unwrap_or_default();
            Argon2::default()
                .hash_password(typed.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|e| ApiError::Internal(e.to_string()))
        })
        .collect::<ApiResult<Vec<String>>>()?;
    sqlx::query("delete from recovery_codes where user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let created: Vec<(OffsetDateTime,)> = sqlx::query_as(
        "insert into recovery_codes (user_id, code_hash) select $1, unnest($2::text[]) returning created_at",
    )
    .bind(user_id)
    .bind(&hashes)
    .fetch_all(&mut *tx)
    .await?;
    let created_at = created.first().map_or_else(OffsetDateTime::now_utc, |(at,)| *at);
    Ok(NewSet { codes, created_at })
}

/// How many of the user's codes are left, and when the set was made (none when there is no set).
pub async fn status(db: &PgPool, user_id: Uuid) -> ApiResult<(i64, Option<OffsetDateTime>)> {
    Ok(sqlx::query_as(
        "select count(*) filter (where used_at is null), max(created_at) from recovery_codes where user_id = $1",
    )
    .bind(user_id)
    .fetch_one(db)
    .await?)
}

/// The unused code of `user` that `typed` is, if any. Always checks [`SET_SIZE`] hashes, against a dummy where the
/// user has fewer codes or is unknown (`None`), so the time it takes says nothing.
pub async fn matching(db: &PgPool, user: Option<Uuid>, typed: &str) -> ApiResult<Option<Uuid>> {
    let unused: Vec<(Uuid, String)> = match user {
        Some(id) => {
            sqlx::query_as(
                "select id, code_hash from recovery_codes where user_id = $1 and used_at is null
                 order by created_at, id limit $2",
            )
            .bind(id)
            .bind(i64::try_from(SET_SIZE).unwrap_or(10))
            .fetch_all(db)
            .await?
        }
        None => Vec::new(),
    };
    let candidate = normalize(typed).unwrap_or_default();
    let mut found = None;
    for i in 0..SET_SIZE {
        let (id, hash) = unused
            .get(i)
            .map_or((None, auth::DUMMY_HASH.as_str()), |(id, hash)| {
                (Some(*id), hash.as_str())
            });
        let same = Argon2::default()
            .verify_password(candidate.as_bytes(), hash)
            .is_ok();
        if same && found.is_none() {
            found = id;
        }
    }
    Ok(found)
}

/// Finishes a login with a recovery code: uses the code, retires the authenticator, ends the user's other sessions
/// and opens a new one, all at once. A code another login claimed first gets `Unauthorized`.
pub async fn recover(db: &PgPool, user_id: Uuid, code_id: Uuid) -> ApiResult<(String, OffsetDateTime)> {
    let mut tx = db.begin().await?;
    let claimed = sqlx::query(
        "update recovery_codes set used_at = now() where id = $1 and user_id = $2 and used_at is null",
    )
    .bind(code_id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    if claimed.rows_affected() == 0 {
        return Err(ApiError::Unauthorized);
    }
    sqlx::query(
        "update users set reenroll_required = true, pending_totp_secret = null, failed_logins = 0,
         locked_until = null where id = $1",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("delete from sessions where user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let session = auth::new_session(&mut *tx, user_id).await?;
    tx.commit().await?;
    Ok(session)
}

/// A new set for a user with a working authenticator, who proves it with a current code. A wrong code counts toward
/// the login lockout, so a stolen session cannot guess its way to a new set.
pub async fn regenerate(db: &PgPool, user_id: Uuid, code: &str) -> ApiResult<NewSet> {
    let (secret, locked): (Vec<u8>, bool) =
        sqlx::query_as("select totp_secret, coalesce(locked_until > now(), false) from users where id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await?;
    if locked {
        return Err(ApiError::Forbidden(
            "too many wrong codes: wait 15 minutes and try again".to_owned(),
        ));
    }
    let Some(step) = totp::code_step(&secret, code)? else {
        auth::record_failure(db, user_id).await?;
        return Err(ApiError::Invalid(WRONG_CODE.to_owned()));
    };
    let mut tx = db.begin().await?;
    // A code already used, for a login or here, is refused like a wrong one (without counting: it was right once).
    if !auth::claim_step(&mut *tx, user_id, step).await? {
        return Err(ApiError::Invalid(WRONG_CODE.to_owned()));
    }
    let set = issue(&mut tx, user_id).await?;
    tx.commit().await?;
    Ok(set)
}

const WRONG_CODE: &str = "the code does not match; wait for the next one and try again";

/// The authenticator a user sets up after a recovery: made on the first call and the same afterwards, until it is
/// confirmed or another recovery login replaces it. Refused when the user's authenticator still works.
pub async fn pending_authenticator(db: &PgPool, user_id: Uuid) -> ApiResult<invites::Enrollment> {
    let row: Option<(Vec<u8>, String)> = sqlx::query_as(
        "update users set pending_totp_secret = coalesce(pending_totp_secret, $2)
         where id = $1 and reenroll_required returning pending_totp_secret, email",
    )
    .bind(user_id)
    .bind(totp::new_secret())
    .fetch_optional(db)
    .await?;
    let (secret, email) = row.ok_or_else(not_recovering)?;
    let uri = totp::enrollment_uri(&email, &secret)?;
    Ok(invites::Enrollment {
        qr_svg: invites::qr_svg(&uri)?,
        secret_base32: totp::secret_base32(&secret)?,
        uri,
    })
}

/// Makes the pending authenticator the user's, given a current code from it; the user's sessions are whole again.
pub async fn confirm_authenticator(db: &PgPool, user_id: Uuid, code: &str) -> ApiResult<()> {
    let (recovering, pending): (bool, Option<Vec<u8>>) =
        sqlx::query_as("select reenroll_required, pending_totp_secret from users where id = $1")
            .bind(user_id)
            .fetch_one(db)
            .await?;
    if !recovering {
        return Err(not_recovering());
    }
    let pending = pending.ok_or_else(|| {
        ApiError::Invalid("open the new authenticator step first, then send its code".to_owned())
    })?;
    let step = totp::code_step(&pending, code)?.ok_or_else(|| ApiError::Invalid(WRONG_CODE.to_owned()))?;
    // Only the secret that was checked: a recovery login in between replaced it, and this code no longer counts.
    let swapped = sqlx::query(
        "update users set totp_secret = pending_totp_secret, pending_totp_secret = null, reenroll_required = false,
         totp_last_step = $2, failed_logins = 0, locked_until = null
         where id = $1 and reenroll_required and pending_totp_secret = $3",
    )
    .bind(user_id)
    .bind(step)
    .bind(&pending)
    .execute(db)
    .await?;
    if swapped.rows_affected() == 0 {
        return Err(ApiError::Invalid(
            "the new authenticator changed: open the step again and scan it".to_owned(),
        ));
    }
    Ok(())
}

fn not_recovering() -> ApiError {
    ApiError::Conflict(
        "this account's authenticator works; a new one is set up only after a login with a recovery code"
            .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_code_reads_back_as_itself() {
        for _ in 0..200 {
            let code = new_code();
            assert_eq!(code.len(), 11, "{code}");
            assert_eq!(&code[5..6], "-", "{code}");
            assert_eq!(normalize(&code).unwrap(), code.replace('-', ""));
        }
    }

    #[test]
    fn typing_forgives_case_spacing_and_look_alikes() {
        assert_eq!(normalize(" AB0C1-DEF2G ").as_deref(), Some("ab0c1def2g"));
        assert_eq!(normalize("ab0c1 def2g").as_deref(), Some("ab0c1def2g"));
        assert_eq!(normalize("abOcI-def2g").as_deref(), Some("ab0c1def2g"));
        assert_eq!(normalize("abLc0-def2g").as_deref(), Some("ab1c0def2g"));
    }

    #[test]
    fn anything_else_is_not_a_code() {
        assert_eq!(normalize("ab0c1-def2"), None, "nine characters");
        assert_eq!(normalize("ab0c1-def2gh"), None, "eleven characters");
        assert_eq!(normalize("ab0c1-defug"), None, "u is not in the alphabet");
        assert_eq!(normalize("123456"), None, "an authenticator code");
        assert_eq!(normalize(""), None);
    }
}
