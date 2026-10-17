use crate::archive_import::decode_error_samples;
use crate::archive_import::{
    archive_outcome, ActivityArchiveImportResponse, ACTIVITY_ARCHIVE_IMPORT_STATUS_FAILED,
    ACTIVITY_ARCHIVE_IMPORT_STATUS_QUEUED, ACTIVITY_ARCHIVE_IMPORT_STATUS_RUNNING,
};
use crate::background_jobs::entities::worker_batches;
use crate::entities::activity_archive_import_jobs;
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveModelTrait, DatabaseConnection};
use sea_orm::{ConnectionTrait, DbErr, QueryFilter, QueryOrder, QuerySelect, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "activity_archive_import_jobs")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub user_storage_key: String,
    pub archive_url: String,
    pub resolved_url: Option<String>,
    pub status: String,
    pub failure_message: Option<String>,
    pub error_samples_json: Option<String>,
    #[sea_orm(default_value = 0)]
    pub total_entries: i32,
    #[sea_orm(default_value = 0)]
    pub supported_entry_count: i32,
    #[sea_orm(default_value = 0)]
    pub imported_count: i32,
    #[sea_orm(default_value = 0)]
    pub duplicate_count: i32,
    #[sea_orm(default_value = 0)]
    pub skipped_unsupported_count: i32,
    #[sea_orm(default_value = 0)]
    pub failed_count: i32,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn recent_for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_desc(Column::CreatedAt)
            .limit(10)
            .all(db)
            .await
    }

    pub async fn find_owned(db: &impl ConnectionTrait, user_id: i32, job_id: i32) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(job_id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }

    pub async fn store_progress(
        db: &impl ConnectionTrait,
        user_id: i32,
        job_id: i32,
        summary: &crate::archive_import::ActivityArchiveImportResponse,
    ) -> Result<(), DbErr> {
        let errors = serde_json::to_string(&summary.error_samples).map_err(|error| DbErr::Custom(error.to_string()))?;
        let result = Self::update_many()
            .set(
                ActiveModel {
                    error_samples_json: Set(Some(errors)),
                    updated_at: Set(Utc::now()),
                    ..Default::default()
                }
                .with_progress(summary),
            )
            .filter(Column::Id.eq(job_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        if result.rows_affected != 1 {
            return Err(DbErr::RecordNotFound("Owned archive job was not found".into()));
        }
        Ok(())
    }
}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Entity {
    pub async fn find_for_batch(db: &impl ConnectionTrait, job_id: i32) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(job_id).one(db).await
    }
}

impl Model {
    pub async fn finish_batch(
        db: &impl ConnectionTrait,
        batch: &worker_batches::Model,
        tasks: &[crate::background_jobs::entities::diagnostic_reads::TaskMetadata],
    ) -> Result<bool, sea_orm::DbErr> {
        if batch.kind != "archive" {
            return Ok(false);
        }
        let id = batch.source["job_id"]
            .as_i64()
            .and_then(|id| i32::try_from(id).ok())
            .ok_or_else(|| sea_orm::DbErr::Custom("Archive job ID missing".into()))?;
        let job = Entity::find_for_batch(db, id)
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound("Archive job missing".into()))?;
        let imports = crate::entities::activity_imports::Entity::archive_statuses(db, id).await?;
        let imported = imports.iter().filter(|status| status.as_str() == "processed").count() as i32;
        let duplicate = imports.iter().filter(|status| status.as_str() == "duplicate").count() as i32;
        let failed = (job.supported_entry_count - imported - duplicate).max(0);
        let task_failed = tasks.iter().any(|task| task.status != "completed");
        let has_failure = task_failed || failed > 0 || job.supported_entry_count == 0;
        let mut errors = decode_error_samples(job.error_samples_json.as_deref());
        errors.extend(tasks.iter().filter_map(|task| task.error.clone()));
        if job.supported_entry_count == 0 && errors.is_empty() {
            errors.push("Archive contained no supported activity files".into());
        }
        errors.truncate(10);
        Entity::update_many()
            .set(ActiveModel {
                imported_count: Set(imported),
                duplicate_count: Set(duplicate),
                failed_count: Set(failed),
                status: Set(if !has_failure {
                    "succeeded"
                } else if imported + duplicate > 0 {
                    "partial"
                } else {
                    "failed"
                }
                .into()),
                failure_message: Set(errors.first().cloned()),
                error_samples_json: Set(Some(
                    serde_json::to_string(&errors).map_err(|error| sea_orm::DbErr::Custom(error.to_string()))?,
                )),
                resolved_url: Set(batch.source["resolved_url"].as_str().map(str::to_owned)),
                finished_at: Set(Some(Utc::now())),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(id))
            .exec(db)
            .await?;
        Ok(has_failure)
    }
}

impl Model {
    pub async fn mark_running(
        db: &DatabaseConnection,
        job: &activity_archive_import_jobs::Model,
    ) -> Result<activity_archive_import_jobs::Model, AppError> {
        activity_archive_import_jobs::ActiveModel {
            id: Set(job.id),
            status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_RUNNING.to_string()),
            failure_message: Set(None),
            error_samples_json: Set(None),
            resolved_url: Set(None),
            started_at: Set(Some(Utc::now())),
            finished_at: Set(None),
            ..Default::default()
        }
        .reset_progress()
        .update(db)
        .await
        .map_err(AppError::from)
    }
}

impl Model {
    pub async fn mark_finished(
        db: &DatabaseConnection,
        job: &activity_archive_import_jobs::Model,
        summary: &ActivityArchiveImportResponse,
    ) -> Result<activity_archive_import_jobs::Model, AppError> {
        activity_archive_import_jobs::ActiveModel {
            id: Set(job.id),
            resolved_url: Set(Some(summary.source.clone())),
            status: Set(archive_outcome(summary).into()),
            failure_message: Set(None),
            error_samples_json: Set(Some(serde_json::to_string(&summary.error_samples).map_err(
                |error| AppError::internal(format!("Failed to serialize archive import errors: {error}")),
            )?)),
            finished_at: Set(Some(Utc::now())),
            ..Default::default()
        }
        .with_progress(summary)
        .update(db)
        .await
        .map_err(AppError::from)
    }
}

impl Model {
    pub async fn mark_failed(
        db: &DatabaseConnection,
        job: &activity_archive_import_jobs::Model,
        resolved_url: Option<String>,
        failure_message: String,
    ) -> Result<activity_archive_import_jobs::Model, AppError> {
        activity_archive_import_jobs::ActiveModel {
            id: Set(job.id),
            resolved_url: Set(resolved_url),
            status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_FAILED.to_string()),
            failure_message: Set(Some(failure_message)),
            finished_at: Set(Some(Utc::now())),
            ..Default::default()
        }
        .update(db)
        .await
        .map_err(AppError::from)
    }
}

impl Model {
    pub async fn queue(
        db: &impl ConnectionTrait,
        user_id: i32,
        user_storage_key: &str,
        archive_url: String,
    ) -> Result<Self, DbErr> {
        activity_archive_import_jobs::ActiveModel {
            user_id: Set(user_id),
            user_storage_key: Set(user_storage_key.to_string()),
            archive_url: Set(archive_url),
            status: Set(ACTIVITY_ARCHIVE_IMPORT_STATUS_QUEUED.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}

impl ActiveModel {
    fn with_progress(self, summary: &ActivityArchiveImportResponse) -> Self {
        Self {
            total_entries: Set(summary.total_entries),
            supported_entry_count: Set(summary.supported_entry_count),
            imported_count: Set(summary.imported_count),
            duplicate_count: Set(summary.duplicate_count),
            skipped_unsupported_count: Set(summary.skipped_unsupported_count),
            failed_count: Set(summary.failed_count),
            ..self
        }
    }

    fn reset_progress(self) -> Self {
        Self {
            total_entries: Set(0),
            supported_entry_count: Set(0),
            imported_count: Set(0),
            duplicate_count: Set(0),
            skipped_unsupported_count: Set(0),
            failed_count: Set(0),
            ..self
        }
    }
}
