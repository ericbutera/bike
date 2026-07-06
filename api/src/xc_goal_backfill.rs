use crate::activity_import_lock::{
    acquire_user_activity_import_lock, load_user_activity_import_lock,
    release_user_activity_import_lock, ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::app_error::AppError;
use crate::entities::user_preferences;
use crate::tasks::TaskQueue;
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

pub const XC_GOAL_BACKFILL_STATUS_QUEUED: &str = "queued";
pub const XC_GOAL_BACKFILL_STATUS_WAITING: &str = "waiting";
pub const XC_GOAL_BACKFILL_STATUS_RUNNING: &str = "running";
pub const XC_GOAL_BACKFILL_STATUS_COMPLETED: &str = "completed";
pub const XC_GOAL_BACKFILL_STATUS_FAILED: &str = "failed";

pub async fn queue_user_xc_goal_backfill(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(String, String), AppError> {
    if let Some(lock) = load_user_activity_import_lock(db, user_id).await? {
        if lock.source == ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL {
            let status = if lock.stage == ACTIVITY_IMPORT_LOCK_STAGE_RUNNING {
                XC_GOAL_BACKFILL_STATUS_RUNNING
            } else {
                XC_GOAL_BACKFILL_STATUS_QUEUED
            };
            set_user_xc_goal_backfill_state(db, user_id, Some(status), None).await?;
            return Ok((status.to_string(), message_for_status(status).to_string()));
        }

        tasks
            .backfill_user_xc_training(user_id)
            .await
            .map_err(|message| {
                AppError::internal(format!("Failed to queue XC training backfill: {message}"))
            })?;
        set_user_xc_goal_backfill_state(db, user_id, Some(XC_GOAL_BACKFILL_STATUS_WAITING), None)
            .await?;
        return Ok((
            XC_GOAL_BACKFILL_STATUS_WAITING.to_string(),
            message_for_status(XC_GOAL_BACKFILL_STATUS_WAITING).to_string(),
        ));
    }

    acquire_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    )
    .await?;

    if let Err(message) = tasks.backfill_user_xc_training(user_id).await {
        release_user_activity_import_lock(
            db,
            user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
        )
        .await?;
        set_user_xc_goal_backfill_state(db, user_id, Some(XC_GOAL_BACKFILL_STATUS_FAILED), None)
            .await?;
        return Err(AppError::internal(format!(
            "Failed to queue XC training backfill: {message}"
        )));
    }

    set_user_xc_goal_backfill_state(db, user_id, Some(XC_GOAL_BACKFILL_STATUS_QUEUED), None)
        .await?;

    Ok((
        XC_GOAL_BACKFILL_STATUS_QUEUED.to_string(),
        message_for_status(XC_GOAL_BACKFILL_STATUS_QUEUED).to_string(),
    ))
}

pub async fn set_user_xc_goal_backfill_state(
    db: &DatabaseConnection,
    user_id: i32,
    status: Option<&str>,
    completed_at: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    let Some(model) = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user_id))
        .one(db)
        .await?
    else {
        return Ok(());
    };

    let mut active_model: user_preferences::ActiveModel = model.into();
    active_model.xc_goal_backfill_status = Set(status.map(str::to_string));
    active_model.xc_goal_backfill_completed_at = Set(completed_at);
    active_model.update(db).await?;
    Ok(())
}

pub async fn clear_user_xc_goal_backfill_state(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), AppError> {
    set_user_xc_goal_backfill_state(db, user_id, None, None).await
}

pub async fn mark_user_xc_goal_backfill_completed(
    db: &DatabaseConnection,
    user_id: i32,
    completed_at: DateTime<Utc>,
) -> Result<(), AppError> {
    set_user_xc_goal_backfill_state(
        db,
        user_id,
        Some(XC_GOAL_BACKFILL_STATUS_COMPLETED),
        Some(completed_at),
    )
    .await
}

pub(crate) fn message_for_status(status: &str) -> &'static str {
    match status {
        XC_GOAL_BACKFILL_STATUS_QUEUED => {
            "XC training backfill queued. Historical rides will repopulate in the background."
        }
        XC_GOAL_BACKFILL_STATUS_WAITING => {
            "XC training backfill is waiting for the current activity processing job to finish."
        }
        XC_GOAL_BACKFILL_STATUS_RUNNING => {
            "XC training backfill is rebuilding historical training metrics."
        }
        XC_GOAL_BACKFILL_STATUS_COMPLETED => {
            "XC training backfill completed. Historical ride metrics are up to date."
        }
        XC_GOAL_BACKFILL_STATUS_FAILED => {
            "XC training backfill failed. Try saving again or queue a user-id backfill from admin analytics."
        }
        _ => "XC training backfill status is unknown.",
    }
}
