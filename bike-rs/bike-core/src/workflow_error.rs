use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug)]
pub struct WorkflowError {
    pub status: StatusCode,
    pub message: String,
    pub errors: Option<HashMap<String, Vec<String>>>,
    pub retry_at: Option<DateTime<Utc>>,
}

impl WorkflowError {
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

impl std::fmt::Display for WorkflowError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for WorkflowError {}

impl From<sea_orm::DbErr> for WorkflowError {
    fn from(error: sea_orm::DbErr) -> Self {
        tracing::error!(error = ?error, "database request failed");
        Self::internal("Database request failed")
    }
}

impl From<std::io::Error> for WorkflowError {
    fn from(error: std::io::Error) -> Self {
        tracing::error!(error = ?error, "file storage request failed");
        Self::internal("File storage request failed")
    }
}

impl From<crate::activity_import_lock::ActivityImportLockError> for WorkflowError {
    fn from(error: crate::activity_import_lock::ActivityImportLockError) -> Self {
        match error {
            crate::activity_import_lock::ActivityImportLockError::Conflict(message) => {
                Self::conflict(message)
            }
            crate::activity_import_lock::ActivityImportLockError::Internal(message) => {
                Self::internal(message)
            }
            crate::activity_import_lock::ActivityImportLockError::Database(error) => {
                Self::from(error)
            }
        }
    }
}

impl From<crate::activity_import_lifecycle::ActivityImportLifecycleError> for WorkflowError {
    fn from(error: crate::activity_import_lifecycle::ActivityImportLifecycleError) -> Self {
        Self::internal(error.message)
    }
}

impl From<crate::segment_support::SegmentSupportError> for WorkflowError {
    fn from(error: crate::segment_support::SegmentSupportError) -> Self {
        Self::internal(error.message)
    }
}

impl From<crate::segment_regeneration::SegmentRegenerationError> for WorkflowError {
    fn from(error: crate::segment_regeneration::SegmentRegenerationError) -> Self {
        Self::internal(error.message)
    }
}
