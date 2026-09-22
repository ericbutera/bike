use crate::entities::activity_imports;
use crate::jobs::{JobQueue, ProcessActivityImportTask};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use kaleido::background_jobs::background_tasks;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use std::fmt;

pub const ACTIVITY_IMPORT_STATUS_PROCESSING: &str = "processing";
pub const ACTIVITY_IMPORT_STAGE_RAW_STORED: &str = "raw_stored";
pub const ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS: i64 = 300;

const MANUAL_UPLOAD_SOURCE: &str = "manual_upload";

#[derive(Debug)]
pub struct ActivityImportRecoveryError {
    pub message: String,
}

impl ActivityImportRecoveryError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ActivityImportRecoveryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ActivityImportRecoveryError {}

impl From<DbErr> for ActivityImportRecoveryError {
    fn from(error: DbErr) -> Self {
        Self::internal(error.to_string())
    }
}

pub async fn recover_stale_manual_activity_imports(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    now: DateTime<Utc>,
) -> Result<usize, ActivityImportRecoveryError> {
    let stale_before = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS);
    let user_ids = activity_imports::Entity::find()
        .select_only()
        .column(activity_imports::Column::UserId)
        .distinct()
        .filter(activity_imports::Column::Source.eq(MANUAL_UPLOAD_SOURCE))
        .filter(activity_imports::Column::Status.eq(ACTIVITY_IMPORT_STATUS_PROCESSING))
        .filter(activity_imports::Column::LastProcessingEventAt.lte(stale_before))
        .into_tuple::<i32>()
        .all(db)
        .await?;

    let mut recovered_count = 0usize;
    for user_id in user_ids {
        recovered_count +=
            recover_stale_manual_activity_imports_for_user(db, tasks, user_id, now).await?;
    }

    Ok(recovered_count)
}

pub async fn recover_abandoned_manual_activity_imports_after_worker_start(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    now: DateTime<Utc>,
) -> Result<usize, ActivityImportRecoveryError> {
    let user_ids = activity_imports::Entity::find()
        .select_only()
        .column(activity_imports::Column::UserId)
        .distinct()
        .filter(activity_imports::Column::Source.eq(MANUAL_UPLOAD_SOURCE))
        .filter(activity_imports::Column::Status.eq(ACTIVITY_IMPORT_STATUS_PROCESSING))
        .into_tuple::<i32>()
        .all(db)
        .await?;

    let mut recovered_count = 0usize;
    for user_id in user_ids {
        recovered_count +=
            recover_manual_activity_imports_for_user(db, tasks, user_id, now, None).await?;
    }

    Ok(recovered_count)
}

pub async fn recover_stale_manual_activity_imports_for_user(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    user_id: i32,
    now: DateTime<Utc>,
) -> Result<usize, ActivityImportRecoveryError> {
    let stale_before = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS);
    recover_manual_activity_imports_for_user(db, tasks, user_id, now, Some(stale_before)).await
}

async fn recover_manual_activity_imports_for_user(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    user_id: i32,
    now: DateTime<Utc>,
    stale_before: Option<DateTime<Utc>>,
) -> Result<usize, ActivityImportRecoveryError> {
    let imports = activity_imports::Entity::find()
        .filter(activity_imports::Column::UserId.eq(user_id))
        .filter(activity_imports::Column::Source.eq(MANUAL_UPLOAD_SOURCE))
        .filter(activity_imports::Column::Status.eq(ACTIVITY_IMPORT_STATUS_PROCESSING))
        .order_by_asc(activity_imports::Column::CreatedAt)
        .all(db)
        .await?;

    let mut recovered_count = 0usize;

    for import in imports {
        if let Some(stale_before) = stale_before {
            let last_event_at = import.last_processing_event_at.unwrap_or(import.updated_at);
            if last_event_at > stale_before {
                continue;
            }
        }

        let active_tasks =
            find_active_process_activity_import_tasks(db, user_id, import.id).await?;
        let has_fresh_processing_task = active_tasks.iter().any(|task| {
            stale_before.is_some_and(|stale_before| {
                task.status == background_tasks::TaskStatus::Processing.as_str()
                    && task.updated_at > stale_before
            })
        });

        if has_fresh_processing_task {
            continue;
        }

        let mut recovered_this_import = false;
        for task in active_tasks
            .iter()
            .filter(|task| task.status == background_tasks::TaskStatus::Processing.as_str())
        {
            reset_background_task_to_pending(db, task).await?;
            recovered_this_import = true;
        }

        let has_pending_task = active_tasks
            .iter()
            .any(|task| task.status == background_tasks::TaskStatus::Pending.as_str());

        if !has_pending_task && !recovered_this_import {
            tasks
                .process_activity_import(user_id, import.id)
                .await
                .map_err(|message| {
                    ActivityImportRecoveryError::internal(format!(
                        "Failed to requeue stale activity import {}: {message}",
                        import.id
                    ))
                })?;
            recovered_this_import = true;
        }

        if has_pending_task || recovered_this_import {
            mark_activity_import_requeued(db, &import, now).await?;
            recovered_count += 1;
        }
    }

    Ok(recovered_count)
}

async fn find_active_process_activity_import_tasks(
    db: &DatabaseConnection,
    user_id: i32,
    import_id: i32,
) -> Result<Vec<background_tasks::Model>, ActivityImportRecoveryError> {
    let tasks = background_tasks::Entity::find()
        .filter(background_tasks::Column::TaskType.eq("process_activity_import"))
        .filter(background_tasks::Column::Status.is_in([
            background_tasks::TaskStatus::Pending.as_str(),
            background_tasks::TaskStatus::Processing.as_str(),
        ]))
        .order_by_desc(background_tasks::Column::CreatedAt)
        .all(db)
        .await?;

    Ok(tasks
        .into_iter()
        .filter(|task| task_targets_activity_import(task, user_id, import_id))
        .collect())
}

fn task_targets_activity_import(
    task: &background_tasks::Model,
    user_id: i32,
    import_id: i32,
) -> bool {
    serde_json::from_value::<ProcessActivityImportTask>(
        task.payload
            .get("data")
            .cloned()
            .unwrap_or_else(|| task.payload.clone()),
    )
    .map(|task| task.user_id == user_id && task.import_id == import_id)
    .unwrap_or(false)
}

async fn reset_background_task_to_pending(
    db: &DatabaseConnection,
    task: &background_tasks::Model,
) -> Result<(), ActivityImportRecoveryError> {
    let mut active: background_tasks::ActiveModel = task.clone().into();
    active.status = Set(background_tasks::TaskStatus::Pending.as_str().to_string());
    active.attempts = Set(0);
    active.error = Set(None);
    active.scheduled_for = Set(None);
    active.started_at = Set(None);
    active.completed_at = Set(None);
    active.result = Set(None);
    active.updated_at = Set(Utc::now());
    active.update(db).await?;

    Ok(())
}

async fn mark_activity_import_requeued(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    now: DateTime<Utc>,
) -> Result<activity_imports::Model, ActivityImportRecoveryError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string());
    active_model.processing_stage = Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.to_string());
    active_model.processing_error = Set(None);
    active_model.last_processing_event_at = Set(Some(now));
    active_model.update(db).await.map_err(Into::into)
}
