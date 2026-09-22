use async_trait::async_trait;
use bike_core::activity_import_lock::{
    acquire_user_activity_import_lock, load_user_activity_import_lock,
    release_user_activity_import_lock, ActivityImportLockError,
    ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL, ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
};
use bike_core::activity_training_analysis::backfill_user_activity_training_analysis_cache;
use bike_core::jobs::{BackfillUserXcTrainingTask, JobQueue};
use bike_core::xc_goal_backfill::{
    mark_user_xc_goal_backfill_completed, set_user_xc_goal_backfill_state, XcGoalBackfillError,
    XC_GOAL_BACKFILL_STATUS_FAILED, XC_GOAL_BACKFILL_STATUS_RUNNING,
    XC_GOAL_BACKFILL_STATUS_WAITING,
};
use chrono::{Duration, Utc};
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::{DatabaseConnection, DbErr};
use std::error::Error;

const XC_BACKFILL_RETRY_DELAY_SECONDS: i64 = 30;
type WorkerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct BackfillUserXcTraining {
    db: DatabaseConnection,
    tasks: JobQueue,
}

impl BackfillUserXcTraining {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tasks: JobQueue::new(db.clone()),
            db,
        }
    }
}

#[async_trait]
impl TaskProcessor for BackfillUserXcTraining {
    fn task_type(&self) -> &str {
        "backfill_user_xc_training"
    }

    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> WorkerResult<()> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: BackfillUserXcTrainingTask = serde_json::from_value(data.clone())?;

        if self.ensure_backfill_lock_or_requeue(&task).await? {
            return Ok(());
        }

        self.mark_running(task.user_id).await?;

        let backfill_result =
            backfill_user_activity_training_analysis_cache(&self.db, task.user_id).await;
        let status_result = self
            .record_backfill_status(task.user_id, &backfill_result)
            .await;
        let release_result = self.release_backfill_lock(task.user_id).await;

        finish_backfill_task(backfill_result, status_result, release_result)
    }
}

impl BackfillUserXcTraining {
    async fn ensure_backfill_lock_or_requeue(
        &self,
        task: &BackfillUserXcTrainingTask,
    ) -> WorkerResult<bool> {
        if let Some(lock) = load_user_activity_import_lock(&self.db, task.user_id)
            .await
            .map_err(worker_activity_import_lock_error)?
        {
            if lock.source != ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL {
                self.mark_waiting_and_requeue(task.user_id).await?;
                return Ok(true);
            }
            return Ok(false);
        }

        acquire_user_activity_import_lock(
            &self.db,
            task.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
            ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
        )
        .await
        .map_err(worker_activity_import_lock_error)?;

        Ok(false)
    }

    async fn mark_waiting_and_requeue(&self, user_id: i32) -> WorkerResult<()> {
        set_user_xc_goal_backfill_state(
            &self.db,
            user_id,
            Some(XC_GOAL_BACKFILL_STATUS_WAITING),
            None,
        )
        .await
        .map_err(worker_xc_goal_backfill_error)?;
        self.tasks
            .backfill_user_xc_training_with_options(
                user_id,
                Some(Utc::now() + Duration::seconds(XC_BACKFILL_RETRY_DELAY_SECONDS)),
                1,
            )
            .await
            .map_err(std::io::Error::other)?;

        Ok(())
    }

    async fn mark_running(&self, user_id: i32) -> WorkerResult<()> {
        set_user_xc_goal_backfill_state(
            &self.db,
            user_id,
            Some(XC_GOAL_BACKFILL_STATUS_RUNNING),
            None,
        )
        .await
        .map_err(worker_xc_goal_backfill_error)
    }

    async fn record_backfill_status(
        &self,
        user_id: i32,
        backfill_result: &Result<usize, DbErr>,
    ) -> Result<(), std::io::Error> {
        match backfill_result {
            Ok(rebuilt_activity_count) => {
                tracing::info!(
                    user_id,
                    rebuilt_activity_count,
                    "completed XC training analysis backfill"
                );
                mark_user_xc_goal_backfill_completed(&self.db, user_id, Utc::now())
                    .await
                    .map_err(status_xc_goal_backfill_error)
            }
            Err(error) => {
                tracing::error!(error = ?error, "database request failed");
                let error_message = "Database request failed";
                set_user_xc_goal_backfill_state(
                    &self.db,
                    user_id,
                    Some(XC_GOAL_BACKFILL_STATUS_FAILED),
                    None,
                )
                .await
                .map_err(status_xc_goal_backfill_error)?;
                Err(std::io::Error::other(error_message))
            }
        }
    }

    async fn release_backfill_lock(&self, user_id: i32) -> Result<(), std::io::Error> {
        release_user_activity_import_lock(
            &self.db,
            user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
        )
        .await
        .map_err(status_activity_import_lock_error)
    }
}

fn finish_backfill_task(
    backfill_result: Result<usize, DbErr>,
    status_result: Result<(), std::io::Error>,
    release_result: Result<(), std::io::Error>,
) -> WorkerResult<()> {
    match (backfill_result, status_result, release_result) {
        (Ok(_), Ok(()), Ok(())) => Ok(()),
        (Err(error), _, _) => Err(worker_activity_training_analysis_error(error)),
        (Ok(_), Err(error), _) => Err(error.into()),
        (Ok(_), Ok(()), Err(error)) => Err(error.into()),
    }
}

fn worker_activity_training_analysis_error(error: DbErr) -> Box<dyn Error + Send + Sync> {
    tracing::error!(error = ?error, "database request failed");
    std::io::Error::other("Database request failed").into()
}

fn worker_activity_import_lock_error(
    error: ActivityImportLockError,
) -> Box<dyn Error + Send + Sync> {
    activity_import_lock_io_error(error).into()
}

fn worker_xc_goal_backfill_error(error: XcGoalBackfillError) -> Box<dyn Error + Send + Sync> {
    xc_goal_backfill_io_error(error).into()
}

fn status_activity_import_lock_error(error: ActivityImportLockError) -> std::io::Error {
    activity_import_lock_io_error(error)
}

fn status_xc_goal_backfill_error(error: XcGoalBackfillError) -> std::io::Error {
    xc_goal_backfill_io_error(error)
}

fn xc_goal_backfill_io_error(error: XcGoalBackfillError) -> std::io::Error {
    match error {
        XcGoalBackfillError::ActivityImportLock(error) => activity_import_lock_io_error(error),
        XcGoalBackfillError::Database(error) => {
            tracing::error!(error = ?error, "database request failed");
            std::io::Error::other("Database request failed")
        }
        XcGoalBackfillError::Queue(message) => {
            std::io::Error::other(format!("Failed to queue XC training backfill: {message}"))
        }
    }
}

fn activity_import_lock_io_error(error: ActivityImportLockError) -> std::io::Error {
    match error {
        ActivityImportLockError::Conflict(message) | ActivityImportLockError::Internal(message) => {
            std::io::Error::other(message)
        }
        ActivityImportLockError::Database(error) => {
            tracing::error!(error = ?error, "database request failed");
            std::io::Error::other("Database request failed")
        }
    }
}
