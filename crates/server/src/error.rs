//! Errors as RFC 9457 problem details (`application/problem+json`).

use axum::Json;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use utoipa::ToSchema;

/// An error response. `title` is fixed per status; `detail` says what to fix, without internals.
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// Always `about:blank`: the status code says it all.
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub title: &'static str,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug)]
pub enum ApiError {
    /// No valid session, or a failed login; the detail never says which part was wrong.
    Unauthorized,
    /// The request is well formed but breaks a rule; the detail names it.
    Invalid(String),
    NotFound,
    /// The row already exists (a unique name, an `external_id` already imported).
    Conflict(String),
    /// Anything unexpected; logged, never shown.
    Internal(String),
}

impl ApiError {
    fn status(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) => StatusCode::CONFLICT,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status();
        let detail = match self {
            ApiError::Unauthorized => Some("a valid session is required".to_owned()),
            ApiError::Invalid(detail) | ApiError::Conflict(detail) => Some(detail),
            ApiError::NotFound => None,
            ApiError::Internal(cause) => {
                eprintln!("internal error: {cause}");
                None
            }
        };
        let problem = Problem {
            kind: "about:blank",
            title: status.canonical_reason().unwrap_or("Error"),
            status: status.as_u16(),
            detail,
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
                Some("23505") => return ApiError::Conflict("it already exists".to_owned()),
                Some("23503") => {
                    return ApiError::Invalid("it refers to something that does not exist".to_owned());
                }
                Some("23514") => return ApiError::Invalid("a value is out of range".to_owned()),
                _ => {}
            }
        }
        ApiError::Internal(error.to_string())
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
