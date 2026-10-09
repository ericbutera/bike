use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use std::fmt;

/// Glass module errors (feature flags, shared utilities)
#[derive(Debug)]
pub struct PlatformError {
    pub code: StatusCode,
    pub message: String,
}

impl fmt::Display for PlatformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PlatformError {}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl PlatformError {
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::FORBIDDEN,
            message: message.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::UNPROCESSABLE_ENTITY,
            message: message.into(),
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::CONFLICT,
            message: message.into(),
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }

    pub fn entity_not_found(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl IntoResponse for PlatformError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.code, body).into_response()
    }
}

// Implement From for common error types
impl From<sea_orm::DbErr> for PlatformError {
    fn from(err: sea_orm::DbErr) -> Self {
        Self::internal_error(format!("Database error: {}", err))
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
