use crate::entities::{activity_imports, integration_events};
use sea_orm::{DatabaseConnection, DbErr};
use serde_json::Value;
use std::fmt;
pub const ACTIVITY_IMPORT_STATUS_PROCESSING: &str = "processing";
pub const ACTIVITY_IMPORT_STATUS_PROCESSED: &str = "processed";
pub const ACTIVITY_IMPORT_STATUS_FAILED: &str = "failed";
pub const ACTIVITY_IMPORT_STATUS_DUPLICATE: &str = "duplicate";
pub const ACTIVITY_IMPORT_STATUS_CANCELED: &str = "canceled";

pub const ACTIVITY_IMPORT_STAGE_RAW_STORED: &str = "raw_stored";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_PARSED: &str = "activity_parsed";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED: &str = "activity_saved";
pub const ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT: &str = "segments_built";
pub const ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT: &str = "segment_analytics_built";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT: &str = "activity_analytics_built";
pub const ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT: &str = "training_analysis_built";
pub const ACTIVITY_IMPORT_STAGE_COMPLETE: &str = "complete";

pub const ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS: i64 = 300;
pub const ACTIVITY_PROCESSING_PROVIDER: &str = "activity_processing";
#[derive(Debug)]
pub struct ActivityImportLifecycleError {
    pub message: String,
}

impl ActivityImportLifecycleError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ActivityImportLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ActivityImportLifecycleError {}

impl From<DbErr> for ActivityImportLifecycleError {
    fn from(error: DbErr) -> Self {
        Self::internal(error.to_string())
    }
}

pub async fn mark_activity_import_failed(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    error_message: &str,
) -> Result<(), ActivityImportLifecycleError> {
    import.clone().mark_failed(db, stage, error_message).await?;
    Ok(())
}
pub async fn record_activity_processing_event(
    db: &impl sea_orm::ConnectionTrait,
    import: &activity_imports::Model,
    event_type: &str,
    level: &str,
    message: String,
    payload: Option<Value>,
) {
    if let Err(error) = integration_events::Entity::record(
        db,
        integration_events::NewEvent {
            user_id: Some(import.user_id),
            provider: ACTIVITY_PROCESSING_PROVIDER.to_string(),
            event_type: event_type.to_string(),
            level: level.to_string(),
            message,
            connection_id: None,
            payload,
        },
    )
    .await
    {
        tracing::warn!(
            error = ?error,
            import_id = import.id,
            event_type,
            "failed to record activity processing event"
        );
    }
}
