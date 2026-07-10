use api::activity_import_pipeline::{
    finalize_activity_import_batch, mark_activity_import_failed, mark_activity_imports_processed,
    process_stored_activity_import, reprocess_activity_from_import, ActivityUploadDeduplication,
    PersistActivityUploadOutcome, ACTIVITY_IMPORT_STATUS_PROCESSING,
};
use api::config::Config;
use api::entities::{activities, activity_imports};
use api::tasks::{ProcessActivityImportTask, TaskQueue};
use async_trait::async_trait;
use chrono::Utc;
use kaleido::background_jobs::worker::TaskProcessor;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::error::Error;

pub struct ProcessActivityImport {
    db: DatabaseConnection,
    tasks: TaskQueue,
    uploads_dir: String,
}

impl ProcessActivityImport {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            tasks: TaskQueue::new(db.clone()),
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

    async fn process(
        &self,
        _task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: ProcessActivityImportTask = serde_json::from_value(data.clone())?;

        let import = activity_imports::Entity::find_by_id(task.import_id)
            .filter(activity_imports::Column::UserId.eq(task.user_id))
            .one(&self.db)
            .await
            .map_err(|error| std::io::Error::other(api::app_error::AppError::from(error).message))?
            .ok_or_else(|| {
                api::app_error::AppError::not_found(format!(
                    "Activity import {} was not found",
                    task.import_id
                ))
            })
            .map_err(|error| std::io::Error::other(error.message))?;

        if import.status != ACTIVITY_IMPORT_STATUS_PROCESSING {
            tracing::info!(
                import_id = import.id,
                status = %import.status,
                "skipping activity import task because the import is no longer processing"
            );
            return Ok(());
        }

        if let Some(activity) = load_activity_for_import(&self.db, task.user_id, &import).await? {
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
                    finalize_activity_import_batch(
                        &self.db,
                        &self.tasks,
                        task.user_id,
                        reprocessed.affected_segment_ids.clone(),
                        Some(reprocessed.fitness_dirty_from_day),
                        Utc::now(),
                    )
                    .await
                    .map_err(|error| std::io::Error::other(error.message))?;
                    mark_activity_imports_processed(&self.db, &[import.id])
                        .await
                        .map_err(|error| std::io::Error::other(error.message))?;
                    return Ok(());
                }
                Err(error) => {
                    mark_activity_import_failed(
                        &self.db,
                        &import,
                        &import.processing_stage,
                        &error,
                    )
                    .await
                    .map_err(|error| std::io::Error::other(error.message))?;
                    return Err(std::io::Error::other(error.message).into());
                }
            }
        }

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
                finalize_activity_import_batch(
                    &self.db,
                    &self.tasks,
                    task.user_id,
                    persisted.affected_segment_ids.clone(),
                    Some(persisted.fitness_dirty_from_day),
                    Utc::now(),
                )
                .await
                .map_err(|error| std::io::Error::other(error.message))?;
                mark_activity_imports_processed(&self.db, &[persisted.import.id])
                    .await
                    .map_err(|error| std::io::Error::other(error.message))?;
                Ok(())
            }
            Ok(PersistActivityUploadOutcome::Duplicate(_duplicate)) => Ok(()),
            Err(error) => {
                mark_activity_import_failed(&self.db, &import, &import.processing_stage, &error)
                    .await
                    .map_err(|error| std::io::Error::other(error.message))?;
                Err(std::io::Error::other(error.message).into())
            }
        }
    }
}

async fn load_activity_for_import(
    db: &DatabaseConnection,
    user_id: i32,
    import: &activity_imports::Model,
) -> Result<Option<activities::Model>, Box<dyn Error + Send + Sync>> {
    if let Some(activity_id) = import.activity_id {
        let activity = activities::Entity::find_by_id(activity_id)
            .filter(activities::Column::UserId.eq(user_id))
            .one(db)
            .await
            .map_err(|error| {
                std::io::Error::other(api::app_error::AppError::from(error).message)
            })?;

        if activity.is_some() {
            return Ok(activity);
        }
    }

    activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::ActivityImportId.eq(Some(import.id)))
        .one(db)
        .await
        .map_err(|error| {
            std::io::Error::other(api::app_error::AppError::from(error).message).into()
        })
}
