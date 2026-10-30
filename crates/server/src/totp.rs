//! The authenticator app's codes (RFC 6238 TOTP: HMAC-SHA-1, 6 digits, 30-second steps, one step of clock drift
//! either way), and what an app needs to enroll a secret.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{ApiError, ApiResult};

const TOTP_STEP: u64 = 30;

/// A secret for an authenticator app: 160 random bits, the size RFC 4226 recommends.
pub fn new_secret() -> Vec<u8> {
    rand::random::<[u8; 20]>().to_vec()
}

/// The secret in base32, for apps where it is typed in.
pub fn secret_base32(secret: &[u8]) -> ApiResult<String> {
    Ok(generator(secret, "user")?.secret().to_base32())
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
    Ok(generator(secret, "user")?
        .check(&digits, now_seconds())
        .and_then(|step| i64::try_from(step).ok()))
}

/// The hyphen-minus and the Unicode hyphens and dashes a copied code may carry between its groups.
pub(crate) const DASHES: [char; 6] = ['-', '\u{2010}', '\u{2011}', '\u{2012}', '\u{2013}', '\u{2014}'];

/// The `otpauth://` URI an authenticator app enrolls from.
pub fn enrollment_uri(email: &str, secret: &[u8]) -> ApiResult<String> {
    generator(secret, email)?
        .to_url()
        .map_err(|e| ApiError::Internal(e.to_string()))
}

fn generator(secret: &[u8], account: &str) -> ApiResult<totp_rs::Totp> {
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
    Ok(generator(secret, "user")?.generate(now_seconds()).to_string())
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
