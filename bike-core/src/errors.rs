use chrono::{DateTime, Utc};
use kaleido::glass::cooldown::CooldownError;
use sea_orm::DbErr;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum BikeCoreError {
    BadRequest(String),
    ValidationField {
        field: String,
        message: String,
    },
    Internal(String),
    Database(DbErr),
    Cooldown {
        message: String,
        retry_after_seconds: Option<i64>,
    },
}

impl BikeCoreError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn validation_field(field: &str, message: impl Into<String>) -> Self {
        Self::ValidationField {
            field: field.to_string(),
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    pub fn retry_at(&self, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match self {
            Self::Cooldown {
                retry_after_seconds,
                ..
            } => retry_after_seconds.map(|seconds| now + chrono::Duration::seconds(seconds)),
            Self::BadRequest(_)
            | Self::ValidationField { .. }
            | Self::Internal(_)
            | Self::Database(_) => None,
        }
    }
}

impl fmt::Display for BikeCoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(message)
            | Self::ValidationField { message, .. }
            | Self::Internal(message)
            | Self::Cooldown { message, .. } => formatter.write_str(message),
            Self::Database(error) => write!(formatter, "database request failed: {error}"),
        }
    }
}

impl Error for BikeCoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::BadRequest(_)
            | Self::ValidationField { .. }
            | Self::Internal(_)
            | Self::Cooldown { .. } => None,
        }
    }
}

impl From<DbErr> for BikeCoreError {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

impl From<CooldownError> for BikeCoreError {
    fn from(error: CooldownError) -> Self {
        Self::Cooldown {
            message: error.message,
            retry_after_seconds: error.retry_after_seconds,
        }
    }
}
