use crate::analytics::{
    mark_segment_activity_changes, mark_user_activity_change, mark_user_fitness_dirty,
};
use crate::entities::{activity_imports, integration_events};
use crate::jobs::JobQueue;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use serde_json::Value;
use std::fmt;

pub const ACTIVITY_IMPORT_STATUS_PROCESSING: &str = "processing";
pub const ACTIVITY_IMPORT_STATUS_PROCESSED: &str = "processed";
pub const ACTIVITY_IMPORT_STATUS_FAILED: &str = "failed";
pub const ACTIVITY_IMPORT_STATUS_DUPLICATE: &str = "duplicate";

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

const INTEGRATION_LEVEL_INFO: &str = "info";
const INTEGRATION_LEVEL_SUCCESS: &str = "success";
const INTEGRATION_LEVEL_ERROR: &str = "error";

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

pub async fn mark_activity_import_processing_stage(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    activity_id: Option<i32>,
) -> Result<activity_imports::Model, ActivityImportLifecycleError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string());
    active_model.processing_stage = Set(stage.to_string());
    active_model.processing_error = Set(None);
    active_model.last_processing_event_at = Set(Some(Utc::now()));
    if let Some(activity_id) = activity_id {
        active_model.activity_id = Set(Some(activity_id));
    }

    let updated = active_model.update(db).await?;
    record_activity_processing_event(
        db,
        &updated,
        "stage_completed",
        INTEGRATION_LEVEL_INFO,
        format!("Activity import {} reached {stage}", updated.id),
        Some(serde_json::json!({
            "import_id": updated.id,
            "activity_id": updated.activity_id,
            "source": updated.source,
            "stage": stage,
        })),
    )
    .await;

    Ok(updated)
}

pub async fn mark_activity_imports_processed(
    db: &DatabaseConnection,
    import_ids: &[i32],
) -> Result<(), ActivityImportLifecycleError> {
    let mut import_ids = import_ids
        .iter()
        .copied()
        .filter(|import_id| *import_id > 0)
        .collect::<Vec<_>>();
    import_ids.sort_unstable();
    import_ids.dedup();

    if import_ids.is_empty() {
        return Ok(());
    }

    let imports = activity_imports::Entity::find()
        .filter(activity_imports::Column::Id.is_in(import_ids.iter().copied()))
        .all(db)
        .await?;

    for import in imports {
        let mut active_model: activity_imports::ActiveModel = import.into();
        active_model.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSED.to_string());
        active_model.processing_stage = Set(ACTIVITY_IMPORT_STAGE_COMPLETE.to_string());
        active_model.processing_error = Set(None);
        active_model.processed_at = Set(Some(Utc::now()));
        active_model.last_processing_event_at = Set(Some(Utc::now()));
        let updated = active_model.update(db).await?;
        record_activity_processing_event(
            db,
            &updated,
            "import_processed",
            INTEGRATION_LEVEL_SUCCESS,
            format!("Activity import {} completed processing", updated.id),
            Some(serde_json::json!({
                "import_id": updated.id,
                "activity_id": updated.activity_id,
                "source": updated.source,
                "stage": ACTIVITY_IMPORT_STAGE_COMPLETE,
            })),
        )
        .await;
    }

    Ok(())
}

pub async fn mark_activity_import_failed(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    error_message: &str,
) -> Result<(), ActivityImportLifecycleError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_FAILED.to_string());
    active_model.processing_stage = Set(stage.to_string());
    active_model.processing_error = Set(Some(error_message.to_string()));
    active_model.processing_attempts = Set(import.processing_attempts.saturating_add(1));
    active_model.last_processing_event_at = Set(Some(Utc::now()));
    let updated = active_model.update(db).await?;

    record_activity_processing_event(
        db,
        &updated,
        "import_failed",
        INTEGRATION_LEVEL_ERROR,
        format!(
            "Activity import {} failed at {stage}: {}",
            updated.id, error_message
        ),
        Some(serde_json::json!({
            "import_id": updated.id,
            "activity_id": updated.activity_id,
            "source": updated.source,
            "stage": stage,
            "error": error_message,
        })),
    )
    .await;

    Ok(())
}

pub async fn mark_activity_import_duplicate(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    duplicate_activity_id: i32,
) -> Result<activity_imports::Model, ActivityImportLifecycleError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_DUPLICATE.to_string());
    active_model.activity_id = Set(Some(duplicate_activity_id));
    active_model.processing_stage = Set(ACTIVITY_IMPORT_STAGE_COMPLETE.to_string());
    active_model.processing_error = Set(None);
    active_model.processed_at = Set(Some(Utc::now()));
    active_model.last_processing_event_at = Set(Some(Utc::now()));
    let updated = active_model.update(db).await?;

    record_activity_processing_event(
        db,
        &updated,
        "import_duplicate",
        INTEGRATION_LEVEL_INFO,
        format!(
            "Activity import {} matched existing activity {}",
            updated.id, duplicate_activity_id
        ),
        Some(serde_json::json!({
            "import_id": updated.id,
            "activity_id": duplicate_activity_id,
            "source": updated.source,
            "stage": ACTIVITY_IMPORT_STAGE_COMPLETE,
        })),
    )
    .await;

    Ok(updated)
}

pub async fn finalize_activity_import_batch(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    user_id: i32,
    mut affected_segment_ids: Vec<i32>,
    fitness_dirty_from_day: Option<NaiveDate>,
    changed_at: DateTime<Utc>,
) -> Result<(), ActivityImportLifecycleError> {
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    if let Some(dirty_from_day) = fitness_dirty_from_day {
        mark_user_fitness_dirty(db, user_id, dirty_from_day, changed_at).await?;
    } else {
        mark_user_activity_change(db, user_id, changed_at).await?;
    }
    if !affected_segment_ids.is_empty() {
        mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    }

    tasks.rebuild_fitness_freshness(user_id).await;

    Ok(())
}

pub async fn record_activity_processing_event(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    event_type: &str,
    level: &str,
    message: String,
    payload: Option<Value>,
) {
    if let Err(error) = (integration_events::ActiveModel {
        user_id: Set(Some(import.user_id)),
        provider: Set(ACTIVITY_PROCESSING_PROVIDER.to_string()),
        event_type: Set(event_type.to_string()),
        level: Set(level.to_string()),
        message: Set(message),
        connection_id: Set(None),
        payload: Set(payload),
        ..Default::default()
    })
    .insert(db)
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
