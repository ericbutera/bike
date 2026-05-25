use api::activity_import_lock::{
    acquire_user_activity_import_lock, load_user_activity_import_lock,
    release_user_activity_import_lock, ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
};
use api::activity_training_analysis::backfill_user_activity_training_analysis_cache;
use api::tasks::{BackfillUserXcTrainingTask, TaskQueue};
use api::xc_goal_backfill::{
    mark_user_xc_goal_backfill_completed, set_user_xc_goal_backfill_state,
    XC_GOAL_BACKFILL_STATUS_FAILED, XC_GOAL_BACKFILL_STATUS_RUNNING,
    XC_GOAL_BACKFILL_STATUS_WAITING,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::DatabaseConnection;
use std::error::Error;

const XC_BACKFILL_RETRY_DELAY_SECONDS: i64 = 30;

pub struct BackfillUserXcTraining {
    db: DatabaseConnection,
    tasks: TaskQueue,
}

impl BackfillUserXcTraining {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tasks: TaskQueue::new(db.clone()),
            db,
        }
    }
}

#[async_trait]
impl TaskProcessor for BackfillUserXcTraining {
    fn task_type(&self) -> &str {
        "backfill_user_xc_training"
    }

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: BackfillUserXcTrainingTask = serde_json::from_value(data.clone())?;

        if let Some(lock) = load_user_activity_import_lock(&self.db, task.user_id)
            .await
            .map_err(|error| std::io::Error::other(error.message))?
        {
            if lock.source != ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL {
                set_user_xc_goal_backfill_state(
                    &self.db,
                    task.user_id,
                    Some(XC_GOAL_BACKFILL_STATUS_WAITING),
                    None,
                )
                .await
                .map_err(|error| std::io::Error::other(error.message))?;
                self.tasks
                    .backfill_user_xc_training_with_options(
                        task.user_id,
                        Some(Utc::now() + Duration::seconds(XC_BACKFILL_RETRY_DELAY_SECONDS)),
                        1,
                    )
                    .await
                    .map_err(std::io::Error::other)?;
                return Ok(());
            }
        } else {
            acquire_user_activity_import_lock(
                &self.db,
                task.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
                ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
            )
            .await
            .map_err(|error| std::io::Error::other(error.message))?;
        }

        set_user_xc_goal_backfill_state(
            &self.db,
            task.user_id,
            Some(XC_GOAL_BACKFILL_STATUS_RUNNING),
            None,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message))?;

        let backfill_result =
            backfill_user_activity_training_analysis_cache(&self.db, task.user_id).await;
        let status_result = match &backfill_result {
            Ok(rebuilt_activity_count) => {
                tracing::info!(
                    user_id = task.user_id,
                    rebuilt_activity_count,
                    "completed XC training analysis backfill"
                );
                mark_user_xc_goal_backfill_completed(&self.db, task.user_id, Utc::now())
                    .await
                    .map_err(|error| std::io::Error::other(error.message))
            }
            Err(error) => {
                let error_message = error.message.clone();
                set_user_xc_goal_backfill_state(
                    &self.db,
                    task.user_id,
                    Some(XC_GOAL_BACKFILL_STATUS_FAILED),
                    None,
                )
                .await
                .map_err(|status_error| std::io::Error::other(status_error.message))?;
                Err(std::io::Error::other(error_message))
            }
        };
        let release_result = release_user_activity_import_lock(
            &self.db,
            task.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
        )
        .await
        .map_err(|error| std::io::Error::other(error.message));

        match (backfill_result, status_result, release_result) {
            (Ok(_), Ok(()), Ok(())) => Ok(()),
            (Err(error), _, _) => Err(std::io::Error::other(error.message).into()),
            (Ok(_), Err(error), _) => Err(error.into()),
            (Ok(_), Ok(()), Err(error)) => Err(error.into()),
        }
    }
}
