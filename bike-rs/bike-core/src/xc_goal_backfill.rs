use crate::activity_import_lock::{
    acquire_user_activity_import_lock, load_user_activity_import_lock,
    release_user_activity_import_lock, ActivityImportLockError,
    ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL, ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::entities::user_preferences;
use crate::jobs::JobQueue;
use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use std::error::Error;
use std::fmt;

pub const XC_GOAL_BACKFILL_STATUS_QUEUED: &str = "queued";
pub const XC_GOAL_BACKFILL_STATUS_WAITING: &str = "waiting";
pub const XC_GOAL_BACKFILL_STATUS_RUNNING: &str = "running";
pub const XC_GOAL_BACKFILL_STATUS_COMPLETED: &str = "completed";
pub const XC_GOAL_BACKFILL_STATUS_FAILED: &str = "failed";

#[derive(Debug)]
pub enum XcGoalBackfillError {
    ActivityImportLock(ActivityImportLockError),
    Database(DbErr),
    Queue(String),
}

impl XcGoalBackfillError {
    pub fn queue(message: impl Into<String>) -> Self {
        Self::Queue(message.into())
    }
}

impl fmt::Display for XcGoalBackfillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActivityImportLock(error) => error.fmt(formatter),
            Self::Database(error) => write!(formatter, "database request failed: {error}"),
            Self::Queue(message) => {
                write!(formatter, "failed to queue XC training backfill: {message}")
            }
        }
    }
}

impl Error for XcGoalBackfillError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ActivityImportLock(error) => Some(error),
            Self::Database(error) => Some(error),
            Self::Queue(_) => None,
        }
    }
}

impl From<ActivityImportLockError> for XcGoalBackfillError {
    fn from(error: ActivityImportLockError) -> Self {
        Self::ActivityImportLock(error)
    }
}

impl From<DbErr> for XcGoalBackfillError {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

pub async fn queue_user_xc_goal_backfill(
    db: &DatabaseConnection,
    tasks: &JobQueue,
    user_id: i32,
) -> Result<(String, String), XcGoalBackfillError> {
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
            .map_err(XcGoalBackfillError::queue)?;
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
        return Err(XcGoalBackfillError::queue(message));
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
) -> Result<(), XcGoalBackfillError> {
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
) -> Result<(), XcGoalBackfillError> {
    set_user_xc_goal_backfill_state(db, user_id, None, None).await
}

pub async fn mark_user_xc_goal_backfill_completed(
    db: &DatabaseConnection,
    user_id: i32,
    completed_at: DateTime<Utc>,
) -> Result<(), XcGoalBackfillError> {
    set_user_xc_goal_backfill_state(
        db,
        user_id,
        Some(XC_GOAL_BACKFILL_STATUS_COMPLETED),
        Some(completed_at),
    )
    .await
}

pub fn message_for_status(status: &str) -> &'static str {
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
