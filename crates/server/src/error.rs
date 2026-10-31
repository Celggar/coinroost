//! Errors as RFC 9457 problem details (`application/problem+json`).

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// An error response. `title` is fixed per status; `detail` says what to fix, without internals.
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// Always `about:blank`: the status code and `code` say it all.
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub title: &'static str,
    pub status: u16,
    /// Which rule failed, stable across releases: clients show their own text for it (RFC 9457 extension member).
    pub code: &'static str,
    /// The request body member the problem is about, as a JSON Pointer fragment (`#/namespace`), when it is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pointer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Why a request was turned down: a stable `code` for clients, the English `detail` for people reading the API, and
/// the body member it is about.
#[derive(Debug)]
pub struct Reason {
    code: &'static str,
    detail: String,
    pointer: Option<String>,
}

impl Reason {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Reason {
        Reason {
            code,
            detail: detail.into(),
            pointer: None,
        }
    }

    /// Ties the reason to a body member; dots go down a level (`detection.markers` is `#/detection/markers`).
    #[must_use]
    pub fn at(mut self, member: &str) -> Reason {
        self.pointer = Some(format!("#/{}", member.replace('.', "/")));
        self
    }
}

#[derive(Debug)]
pub enum ApiError {
    /// No valid session, or a failed login; the detail never says which part was wrong.
    Unauthorized,
    /// The caller is known but may not do this (a read-only token asking to write); the reason says why.
    Forbidden(Reason),
    /// The request is well formed but breaks a rule; the reason names it.
    Invalid(Reason),
    NotFound,
    /// The row already exists (a unique name, an `external_id` already imported).
    Conflict(Reason),
    /// What the request was based on has changed (`If-Match` no longer matches); the reason says what to do.
    PreconditionFailed(Reason),
    /// The request must say what it was based on (`If-Match`).
    PreconditionRequired(Reason),
    /// Anything unexpected; logged, never shown.
    Internal(String),
}

impl ApiError {
    /// A 422 for a rule the request breaks.
    pub fn invalid(code: &'static str, detail: impl Into<String>) -> ApiError {
        ApiError::Invalid(Reason::new(code, detail))
    }

    fn status(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::PreconditionFailed(_) => StatusCode::PRECONDITION_FAILED,
            ApiError::PreconditionRequired(_) => StatusCode::PRECONDITION_REQUIRED,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// The reason, or the code and detail that stand for it.
    fn into_reason(self) -> Reason {
        match self {
            ApiError::Unauthorized => Reason::new("unauthorized", "a valid session is required"),
            ApiError::Forbidden(reason)
            | ApiError::Invalid(reason)
            | ApiError::Conflict(reason)
            | ApiError::PreconditionFailed(reason)
            | ApiError::PreconditionRequired(reason) => reason,
            ApiError::NotFound => Reason::new("not_found", ""),
            ApiError::Internal(cause) => {
                eprintln!("internal error: {cause}");
                Reason::new("internal", "")
            }
        }
    }
}

/// For the console: the detail, or what the status means.
impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Unauthorized => f.write_str("not authorized"),
            ApiError::Forbidden(reason)
            | ApiError::Invalid(reason)
            | ApiError::Conflict(reason)
            | ApiError::PreconditionFailed(reason)
            | ApiError::PreconditionRequired(reason) => f.write_str(&reason.detail),
            ApiError::Internal(detail) => f.write_str(detail),
            ApiError::NotFound => f.write_str("not found"),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let reason = self.into_reason();
        let problem = Problem {
            kind: "about:blank",
            title: status.canonical_reason().unwrap_or("Error"),
            status: status.as_u16(),
            code: reason.code,
            pointer: reason.pointer,
            detail: Some(reason.detail).filter(|d| !d.is_empty()),
        };
        let mut response = (status, Json(problem)).into_response();
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> ApiError {
        if let sqlx::Error::Database(db) = &error {
            // Unique violations are conflicts; foreign key and check violations mean a reference or value the
            // household does not have.
            match db.code().as_deref() {
                Some("23505") => return ApiError::Conflict(taken(db.constraint())),
                Some("23503") => {
                    return ApiError::invalid(
                        "unknown_reference",
                        "it refers to something that does not exist",
                    );
                }
                Some("23514") => return ApiError::invalid("out_of_range", "a value is out of range"),
                _ => {}
            }
        }
        ApiError::Internal(error.to_string())
    }
}

/// An `already_exists` conflict, tied to the column a household-wide unique key names
/// (`accounts_household_id_namespace_key` is about `namespace`).
fn taken(constraint: Option<&str>) -> Reason {
    let reason = Reason::new("already_exists", "it already exists");
    match constraint
        .and_then(|c| c.split_once("_household_id_"))
        .and_then(|(_, rest)| rest.strip_suffix("_key"))
    {
        Some(column) => reason.at(column),
        None => reason,
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
