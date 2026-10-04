use crate::auth::entities::cooldowns;
use axum::http::StatusCode;
use chrono::Utc;
use sea_orm::DatabaseConnection;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackoffStrategy {
    Simple,
    Exponential {
        base_seconds: i64,
        max_attempts: i32,
    },
}

#[derive(Debug)]
pub struct CooldownError {
    pub code: StatusCode,
    pub message: String,
    pub retry_after_seconds: Option<i64>,
}

impl CooldownError {
    pub fn too_many_requests(message: impl Into<String>, retry_after_seconds: Option<i64>) -> Self {
        Self {
            code: StatusCode::TOO_MANY_REQUESTS,
            message: message.into(),
            retry_after_seconds,
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            retry_after_seconds: None,
        }
    }
}

impl fmt::Display for CooldownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for CooldownError {}

impl From<sea_orm::DbErr> for CooldownError {
    fn from(err: sea_orm::DbErr) -> Self {
        CooldownError::internal_error(format!("Database error: {}", err))
    }
}

pub struct CooldownService;

impl CooldownService {
    pub fn calculate_exponential_backoff(
        base_seconds: i64,
        attempts: i32,
        max_attempts: i32,
    ) -> i64 {
        if attempts >= max_attempts {
            2_i64.pow(max_attempts as u32) * base_seconds
        } else {
            2_i64.pow(attempts as u32) * base_seconds
        }
    }

    pub async fn check(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
        strategy: BackoffStrategy,
        simple_duration_seconds: i64,
        too_many_requests_message: impl Fn(i64) -> String,
    ) -> Result<(), CooldownError> {
        let existing = cooldowns::Model::find_by(db, subject_type, subject_id, action).await?;

        if let Some(rec) = &existing {
            if let Some(last) = rec.last_run {
                let elapsed = Utc::now().signed_duration_since(last).num_seconds();

                let cooldown_secs = match strategy {
                    BackoffStrategy::Simple => simple_duration_seconds,
                    BackoffStrategy::Exponential {
                        base_seconds,
                        max_attempts,
                    } => Self::calculate_exponential_backoff(
                        base_seconds,
                        rec.attempt_count,
                        max_attempts,
                    ),
                };

                if elapsed < cooldown_secs {
                    let retry_after = cooldown_secs - elapsed;
                    return Err(CooldownError::too_many_requests(
                        too_many_requests_message(retry_after),
                        Some(retry_after),
                    ));
                }
            }
        }

        Ok(())
    }

    pub async fn record_failure(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<(), CooldownError> {
        cooldowns::Model::increment_attempts(db, subject_type, subject_id, action)
            .await
            .map_err(|e| {
                CooldownError::internal_error(format!("Failed to record cooldown failure: {}", e))
            })?;
        Ok(())
    }

    pub async fn reset(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<(), CooldownError> {
        cooldowns::Model::reset_attempts(db, subject_type, subject_id, action)
            .await
            .map_err(|e| {
                CooldownError::internal_error(format!("Failed to reset cooldown: {}", e))
            })?;
        Ok(())
    }

    pub async fn update(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<(), CooldownError> {
        cooldowns::Model::upsert_last_run(db, subject_type, subject_id, action, Utc::now())
            .await
            .map_err(|e| {
                CooldownError::internal_error(format!("Failed to update cooldown: {}", e))
            })?;
        Ok(())
    }

    pub async fn check_and_update(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
        strategy: BackoffStrategy,
        simple_duration_seconds: i64,
        too_many_requests_message: impl Fn(i64) -> String,
    ) -> Result<(), CooldownError> {
        Self::check(
            db,
            subject_type,
            subject_id,
            action,
            strategy,
            simple_duration_seconds,
            too_many_requests_message,
        )
        .await?;
        Self::update(db, subject_type, subject_id, action).await?;
        Ok(())
    }
}
