//! Checks on request values, shared by the handlers. Each returns the cleaned value or a 422 that names the field.

use crate::error::{ApiError, ApiResult};

/// Trimmed text of 1 to `max` characters.
pub fn text(field: &str, value: &str, max: usize) -> ApiResult<String> {
    let value = value.trim();
    let length = value.chars().count();
    if length == 0 || length > max {
        return Err(ApiError::Invalid(format!("{field} needs 1 to {max} characters")));
    }
    Ok(value.to_owned())
}

/// An active ISO 4217 code, upper case.
pub fn currency(code: &str) -> ApiResult<String> {
    let code = code.trim().to_ascii_uppercase();
    iso_currency::Currency::from_code(&code)
        .map(|_| code.clone())
        .ok_or_else(|| ApiError::Invalid(format!("{code} is not an ISO 4217 currency code")))
}

/// Digits after the decimal point in the currency's minor unit (2 for USD).
pub fn minor_digits(code: &str) -> Option<u16> {
    iso_currency::Currency::from_code(code).and_then(iso_currency::Currency::exponent)
}

/// The key that goes into every `external_id` of an account: lower case letters, digits, `_` and `-`.
pub fn namespace(value: &str) -> ApiResult<String> {
    let value = value.trim();
    let valid = (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
        && value.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric());
    if valid {
        Ok(value.to_owned())
    } else {
        Err(ApiError::Invalid(
            "namespace needs 1 to 64 lower case letters, digits, '_' or '-', starting with a letter or digit"
                .to_owned(),
        ))
    }
}
