use async_trait::async_trait;
use bike_core::activity_import_lifecycle::{
    finalize_activity_import_batch, mark_activity_import_failed, mark_activity_imports_processed,
    ACTIVITY_IMPORT_STATUS_PROCESSING,
};
use bike_core::activity_import_pipeline::{
    process_stored_activity_import, reprocess_activity_from_import, ActivityUploadDeduplication,
    PersistActivityUploadOutcome,
};
use bike_core::config::Config;
use bike_core::entities::{activities, activity_imports};
use bike_core::jobs::{JobQueue, ProcessActivityImportTask};
use bike_core::workflow_error::WorkflowError;
use chrono::{NaiveDate, Utc};
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::error::Error;

type WorkerResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub struct ProcessActivityImport {
    db: DatabaseConnection,
    tasks: JobQueue,
    uploads_dir: String,
}

impl ProcessActivityImport {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tasks: JobQueue::new(db.clone()),
            db,
            uploads_dir: Config::get().uploads_dir.clone(),
        }
    }
}

#[async_trait]
impl TaskProcessor for ProcessActivityImport {
    fn task_type(&self) -> &str {
        "process_activity_import"
    }

    async fn process(&self, _task_id: i32, payload: serde_json::Value) -> WorkerResult<()> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: ProcessActivityImportTask = serde_json::from_value(data.clone())?;
        let import = load_import_for_task(&self.db, &task).await?;

        if should_skip_import_task(&import) {
            return Ok(());
        }

        if self
            .process_existing_activity_import(&task, &import)
            .await?
        {
            return Ok(());
        }

        self.process_new_activity_import(&task, import).await
    }
}

impl ProcessActivityImport {
    async fn process_existing_activity_import(
        &self,
        task: &ProcessActivityImportTask,
        import: &activity_imports::Model,
    ) -> WorkerResult<bool> {
        let Some(activity) = load_activity_for_import(&self.db, task.user_id, import).await? else {
            return Ok(false);
        };

        match reprocess_activity_from_import(
            &self.db,
            &self.uploads_dir,
            task.user_id,
            activity,
            import.clone(),
            None,
        )
        .await
        {
            Ok(reprocessed) => {
                self.finalize_import(
                    task.user_id,
                    import.id,
                    reprocessed.affected_segment_ids,
                    Some(reprocessed.fitness_dirty_from_day),
                )
                .await?;
                Ok(true)
            }
            Err(error) => fail_activity_import(&self.db, import, error).await,
        }
    }

    async fn process_new_activity_import(
        &self,
        task: &ProcessActivityImportTask,
        import: activity_imports::Model,
    ) -> WorkerResult<()> {
        match process_stored_activity_import(
            &self.db,
            &self.uploads_dir,
            task.user_id,
            import.clone(),
            ActivityUploadDeduplication::Enabled,
            None,
        )
        .await
        {
            Ok(PersistActivityUploadOutcome::Imported(persisted)) => {
                self.finalize_import(
                    task.user_id,
                    persisted.import.id,
                    persisted.affected_segment_ids,
                    Some(persisted.fitness_dirty_from_day),
                )
                .await
            }
            Ok(PersistActivityUploadOutcome::Duplicate(_duplicate)) => Ok(()),
            Err(error) => fail_activity_import(&self.db, &import, error).await,
        }
    }

    async fn finalize_import(
        &self,
        user_id: i32,
        import_id: i32,
        affected_segment_ids: Vec<i32>,
        fitness_dirty_from_day: Option<NaiveDate>,
    ) -> WorkerResult<()> {
        finalize_activity_import_batch(
            &self.db,
            &self.tasks,
            user_id,
            affected_segment_ids,
            fitness_dirty_from_day,
            Utc::now(),
        )
        .await
        .map_err(worker_core_error)?;
        mark_activity_imports_processed(&self.db, &[import_id])
            .await
            .map_err(worker_core_error)
    }
}

async fn load_import_for_task(
    db: &DatabaseConnection,
    task: &ProcessActivityImportTask,
) -> WorkerResult<activity_imports::Model> {
    activity_imports::Entity::find_by_id(task.import_id)
        .filter(activity_imports::Column::UserId.eq(task.user_id))
        .one(db)
        .await
        .map_err(worker_db_error)?
        .ok_or_else(|| worker_error(format!("Activity import {} was not found", task.import_id)))
}

fn should_skip_import_task(import: &activity_imports::Model) -> bool {
    if import.status == ACTIVITY_IMPORT_STATUS_PROCESSING {
        return false;
    }

    tracing::info!(
        import_id = import.id,
        status = %import.status,
        "skipping activity import task because the import is no longer processing"
    );
    true
}

async fn fail_activity_import<T>(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    error: WorkflowError,
) -> WorkerResult<T> {
    mark_activity_import_failed(db, import, &import.processing_stage, &error.message)
        .await
        .map_err(worker_core_error)?;
    Err(worker_workflow_error(error))
}

fn worker_workflow_error(error: WorkflowError) -> Box<dyn Error + Send + Sync> {
    worker_error(error.message)
}

fn worker_db_error(error: sea_orm::DbErr) -> Box<dyn Error + Send + Sync> {
    worker_error(format!("database request failed: {error}"))
}

fn worker_core_error(
    error: bike_core::activity_import_lifecycle::ActivityImportLifecycleError,
) -> Box<dyn Error + Send + Sync> {
    worker_error(error.message)
}

fn worker_error(message: impl Into<String>) -> Box<dyn Error + Send + Sync> {
    std::io::Error::other(message.into()).into()
}

async fn load_activity_for_import(
    db: &DatabaseConnection,
    user_id: i32,
    import: &activity_imports::Model,
) -> WorkerResult<Option<activities::Model>> {
    if let Some(activity_id) = import.activity_id {
        let activity = activities::Entity::find_by_id(activity_id)
            .filter(activities::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(worker_db_error)?;

        if activity.is_some() {
            return Ok(activity);
        }
    }

    activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::ActivityImportId.eq(Some(import.id)))
        .one(db)
        .await
        .map_err(worker_db_error)
}
