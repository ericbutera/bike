use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiErrorResponse {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
    pub errors: Option<HashMap<String, Vec<String>>>,
    pub retry_at: Option<DateTime<Utc>>,
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            errors: None,
            retry_at: None,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            message: message.into(),
            errors: None,
            retry_at: None,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            message: message.into(),
            errors: None,
            retry_at: None,
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
            errors: None,
            retry_at: None,
        }
    }

    pub fn too_many_requests(message: impl Into<String>, retry_at: Option<DateTime<Utc>>) -> Self {
        Self {
            status: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
            errors: None,
            retry_at,
        }
    }

    pub fn payload_too_large(field: &str, message: impl Into<String>) -> Self {
        Self::field_error(StatusCode::PAYLOAD_TOO_LARGE, field, message)
    }

    pub fn validation_field(field: &str, message: impl Into<String>) -> Self {
        Self::field_error(StatusCode::BAD_REQUEST, field, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            errors: None,
            retry_at: None,
        }
    }

    fn field_error(status: StatusCode, field: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        let mut errors = HashMap::new();
        errors.insert(field.to_string(), vec![message.clone()]);

        Self {
            status,
            message,
            errors: Some(errors),
            retry_at: None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ApiErrorResponse {
            message: self.message,
            errors: self.errors,
            retry_at: self.retry_at,
        };

        (self.status, Json(body)).into_response()
    }
}

impl From<sea_orm::DbErr> for AppError {
    fn from(error: sea_orm::DbErr) -> Self {
        tracing::error!(error = ?error, "database request failed");
        Self::internal("Database request failed")
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        tracing::error!(error = ?error, "file storage request failed");
        Self::internal("File storage request failed")
    }
}
