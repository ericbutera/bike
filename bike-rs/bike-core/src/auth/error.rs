use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use std::fmt;

/// Authentication and authorization errors
#[derive(Debug)]
pub struct AuthError {
    pub code: StatusCode,
    pub message: String,
    pub retry_after_seconds: Option<i64>,
}

impl AuthError {
    pub fn too_many_requests(message: impl Into<String>, retry_after_seconds: Option<i64>) -> Self {
        Self {
            code: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
            retry_after_seconds,
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AuthError {}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl AuthError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::BAD_REQUEST,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::UNAUTHORIZED,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::FORBIDDEN,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::UNPROCESSABLE_ENTITY,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::CONFLICT,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            retry_after_seconds: None,
        }
    }

    pub fn entity_not_found(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::NOT_FOUND,
            message: message.into(),
            retry_after_seconds: None,
        }
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        if let Some(secs) = self.retry_after_seconds {
            let mut res = (self.code, body).into_response();
            if let Ok(hv) = header::HeaderValue::from_str(&secs.to_string()) {
                res.headers_mut().insert(header::RETRY_AFTER, hv);
            }
            res
        } else {
            (self.code, body).into_response()
        }
    }
}

impl From<sea_orm::DbErr> for AuthError {
    fn from(err: sea_orm::DbErr) -> Self {
        AuthError::internal_error(format!("Database error: {}", err))
    }
}

impl From<validator::ValidationErrors> for AuthError {
    fn from(err: validator::ValidationErrors) -> Self {
        AuthError::validation(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for AuthError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AuthError::unauthorized(format!("Token error: {}", err))
    }
}
