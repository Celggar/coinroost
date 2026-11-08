//! Checks on request values, shared by the handlers. Each returns the cleaned value or a 422 that names the field.

use crate::error::{ApiError, ApiResult, Reason};

/// Trimmed text of 1 to `max` characters.
pub fn text(field: &str, value: &str, max: usize) -> ApiResult<String> {
    let value = value.trim();
    let length = value.chars().count();
    if length == 0 || length > max {
        return Err(ApiError::Invalid(
            Reason::new("text_length", format!("{field} needs 1 to {max} characters")).at(field),
        ));
    }
    Ok(value.to_owned())
}

/// An active ISO 4217 code, upper case.
pub fn currency(code: &str) -> ApiResult<String> {
    let code = code.trim().to_ascii_uppercase();
    iso_currency::Currency::from_code(&code)
        .map(|_| code.clone())
        .ok_or_else(|| {
            ApiError::Invalid(
                Reason::new(
                    "currency_unknown",
                    format!("{code} is not an ISO 4217 currency code"),
                )
                .at("currency"),
            )
        })
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
        Err(ApiError::Invalid(Reason::new("namespace_format", "namespace needs 1 to 64 lower case letters, digits, '_' or '-', starting with a letter or digit"
                .to_owned(),).at("namespace")))
    }
}

/// A search text, trimmed: 1 to 200 characters, or `None` when no search is asked for.
pub fn search(q: Option<&str>) -> ApiResult<Option<&str>> {
    let text = q.map(str::trim);
    if text.is_some_and(|q| q.is_empty() || q.chars().count() > 200) {
        return Err(ApiError::invalid(
            "search_length",
            "q has 1 to 200 characters".to_owned(),
        ));
    }
    Ok(text)
}

/// How many items a page holds: 1 to `max`.
pub fn limit(limit: i64, max: i64) -> ApiResult<i64> {
    if !(1..=max).contains(&limit) {
        return Err(ApiError::invalid(
            "limit_out_of_range",
            format!("limit must be between 1 and {max}"),
        ));
    }
    Ok(limit)
}
