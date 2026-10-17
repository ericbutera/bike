use crate::entities::user_preferences;
use crate::jobs::JobQueue;
use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, DbErr};
use std::error::Error;
use std::fmt;

pub const XC_GOAL_BACKFILL_STATUS_QUEUED: &str = "queued";
pub const XC_GOAL_BACKFILL_STATUS_WAITING: &str = "waiting";
pub const XC_GOAL_BACKFILL_STATUS_RUNNING: &str = "running";
pub const XC_GOAL_BACKFILL_STATUS_COMPLETED: &str = "completed";
pub const XC_GOAL_BACKFILL_STATUS_FAILED: &str = "failed";

#[derive(Debug)]
pub enum XcGoalBackfillError {
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
            Self::Database(error) => Some(error),
            Self::Queue(_) => None,
        }
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
    set_user_xc_goal_backfill_state(db, user_id, Some(XC_GOAL_BACKFILL_STATUS_QUEUED), None).await?;
    if let Err(message) = tasks.backfill_user_xc_training(user_id).await {
        set_user_xc_goal_backfill_state(db, user_id, Some(XC_GOAL_BACKFILL_STATUS_FAILED), None).await?;
        return Err(XcGoalBackfillError::queue(message));
    }
    Ok((
        XC_GOAL_BACKFILL_STATUS_QUEUED.into(),
        message_for_status(XC_GOAL_BACKFILL_STATUS_QUEUED).into(),
    ))
}

pub async fn set_user_xc_goal_backfill_state(
    db: &DatabaseConnection,
    user_id: i32,
    status: Option<&str>,
    completed_at: Option<DateTime<Utc>>,
) -> Result<(), XcGoalBackfillError> {
    let Some(model) = user_preferences::Model::for_user(db, user_id).await? else {
        return Ok(());
    };

    model.set_goal_backfill(db, status, completed_at).await?;
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
    set_user_xc_goal_backfill_state(db, user_id, Some(XC_GOAL_BACKFILL_STATUS_COMPLETED), Some(completed_at)).await
}

pub fn message_for_status(status: &str) -> &'static str {
    match status {
        XC_GOAL_BACKFILL_STATUS_QUEUED => {
            "XC training backfill queued. Historical rides will repopulate in the background."
        }
        XC_GOAL_BACKFILL_STATUS_WAITING => {
            "XC training backfill is waiting for the current activity processing job to finish."
        }
        XC_GOAL_BACKFILL_STATUS_RUNNING => "XC training backfill is rebuilding historical training metrics.",
        XC_GOAL_BACKFILL_STATUS_COMPLETED => "XC training backfill completed. Historical ride metrics are up to date.",
        XC_GOAL_BACKFILL_STATUS_FAILED => {
            "XC training backfill failed. Try saving again or queue a user-id backfill from admin analytics."
        }
        _ => "XC training backfill status is unknown.",
    }
}
