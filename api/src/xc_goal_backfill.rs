use crate::app_error::AppError;
use crate::tasks::TaskQueue;
use bike_core::xc_goal_backfill as core_xc_goal_backfill;
use bike_core::xc_goal_backfill::XcGoalBackfillError;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

pub use bike_core::xc_goal_backfill::{
    message_for_status, XC_GOAL_BACKFILL_STATUS_COMPLETED, XC_GOAL_BACKFILL_STATUS_FAILED,
    XC_GOAL_BACKFILL_STATUS_QUEUED, XC_GOAL_BACKFILL_STATUS_RUNNING,
    XC_GOAL_BACKFILL_STATUS_WAITING,
};

impl From<XcGoalBackfillError> for AppError {
    fn from(error: XcGoalBackfillError) -> Self {
        match error {
            XcGoalBackfillError::ActivityImportLock(error) => Self::from(error),
            XcGoalBackfillError::Database(error) => Self::from(error),
            XcGoalBackfillError::Queue(message) => {
                Self::internal(format!("Failed to queue XC training backfill: {message}"))
            }
        }
    }
}

pub async fn queue_user_xc_goal_backfill(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(String, String), AppError> {
    core_xc_goal_backfill::queue_user_xc_goal_backfill(db, tasks, user_id)
        .await
        .map_err(AppError::from)
}

pub async fn set_user_xc_goal_backfill_state(
    db: &DatabaseConnection,
    user_id: i32,
    status: Option<&str>,
    completed_at: Option<DateTime<Utc>>,
) -> Result<(), AppError> {
    core_xc_goal_backfill::set_user_xc_goal_backfill_state(db, user_id, status, completed_at)
        .await
        .map_err(AppError::from)
}

pub async fn clear_user_xc_goal_backfill_state(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), AppError> {
    core_xc_goal_backfill::clear_user_xc_goal_backfill_state(db, user_id)
        .await
        .map_err(AppError::from)
}

pub async fn mark_user_xc_goal_backfill_completed(
    db: &DatabaseConnection,
    user_id: i32,
    completed_at: DateTime<Utc>,
) -> Result<(), AppError> {
    core_xc_goal_backfill::mark_user_xc_goal_backfill_completed(db, user_id, completed_at)
        .await
        .map_err(AppError::from)
}
