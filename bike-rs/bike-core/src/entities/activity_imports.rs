use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, QueryFilter, QueryOrder, QuerySelect, Set, TryIntoModel};

pub const ACTIVITY_IMPORT_VERSION_LEGACY: i32 = 1;
pub const ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE: i32 = 2;
pub const ACTIVITY_IMPORT_VERSION_CURRENT: i32 = ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE;
pub const STRAVA_SOURCE: &str = "strava_sync";

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "activity_imports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    #[sea_orm(default_expr = "Expr::value(ACTIVITY_IMPORT_VERSION_CURRENT)")]
    pub import_version: i32,
    pub source: String,
    pub format: String,
    pub status: String,
    pub activity_id: Option<i32>,
    pub archive_job_id: Option<i32>,
    #[sea_orm(default_value = "complete")]
    pub processing_stage: String,
    pub processing_error: Option<String>,
    #[sea_orm(default_value = 0)]
    pub processing_attempts: i32,
    pub processed_at: Option<DateTime<Utc>>,
    pub last_processing_event_at: Option<DateTime<Utc>>,
    pub original_filename: String,
    pub storage_path: String,
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Model {
    pub(crate) async fn complete_native_reprocess(
        db: &impl ConnectionTrait,
        execution: &crate::bike_tasks::ImportExecution,
    ) -> Result<(), crate::workflow_error::WorkflowError> {
        let source = &execution.command.source;
        let changed = Entity::update_many()
            .set(ActiveModel {
                status: Set("processed".into()),
                processing_stage: Set("complete".into()),
                processing_error: Set(None),
                processed_at: Set(Some(Utc::now())),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(source.activity_import_id))
            .filter(Column::UserId.eq(source.user_id))
            .exec(db)
            .await?;
        if changed.rows_affected != 1 {
            return Err(crate::workflow_error::WorkflowError::conflict(
                "Reprocessed import disappeared",
            ));
        }
        Ok(())
    }
}

impl Entity {
    pub async fn recent_manual_uploads(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.eq(crate::activity_import_lock::ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD))
            .order_by_desc(Column::CreatedAt)
            .limit(25)
            .all(db)
            .await
    }

    pub async fn native_strava_source(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
        existing_import_id: Option<i32>,
        object: &crate::strava::objects::ObjectRef,
        upload: &crate::activity_import_pipeline::ActivityUploadPayload,
    ) -> Result<Model, DbErr> {
        let active = Self::active_for_strava(db, event.bike_user_id, existing_import_id).await?;
        let active = active.with_strava_source(object, upload);
        let import = active.save(db).await?.try_into_model()?;
        super::activity_import_artifacts::Entity::store_strava_payload(db, &import, object.path.clone(), upload)
            .await?;
        Ok(import)
    }

    async fn active_for_strava(
        db: &impl ConnectionTrait,
        user_id: i32,
        existing_import_id: Option<i32>,
    ) -> Result<ActiveModel, DbErr> {
        let existing = match existing_import_id {
            Some(id) => Self::find_owned(db, user_id, id).await?,
            None => None,
        };
        Ok(ActiveModel {
            user_id: Set(user_id),
            ..existing.map(ActiveModel::from).unwrap_or_default()
        })
    }
    pub async fn storage_paths(db: &impl ConnectionTrait) -> Result<Vec<String>, DbErr> {
        Self::find()
            .select_only()
            .column(Column::StoragePath)
            .into_tuple()
            .all(db)
            .await
    }

    pub async fn metrics_counts(
        db: &DatabaseConnection,
        now: DateTime<Utc>,
    ) -> Result<crate::platform::aggregator::StoredRecordCounts, DbErr> {
        crate::platform::aggregator::Aggregator::stored_records::<Entity>(db, Column::CreatedAt, now).await
    }

    pub async fn find_for_administration(db: &impl ConnectionTrait, import_id: i32) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(import_id).one(db).await
    }

    pub async fn processing_for_user(
        db: &impl ConnectionTrait,
        user_id: i32,
        source: Option<&str>,
    ) -> Result<Vec<Model>, DbErr> {
        let mut query = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Status.eq("processing"))
            .filter(Column::Source.ne(STRAVA_SOURCE));
        if let Some(source) = source {
            query = query.filter(Column::Source.eq(source));
        }
        query.order_by_asc(Column::CreatedAt).all(db).await
    }

    pub async fn history(
        db: &impl ConnectionTrait,
        user_id: i32,
        page: u64,
        source: Option<&str>,
        status: Option<&str>,
        archive_job_id: Option<i32>,
    ) -> Result<(Vec<Model>, u64), DbErr> {
        let mut query = Self::find().filter(Column::UserId.eq(user_id));
        if let Some(source) = source {
            query = query.filter(Column::Source.eq(source));
        }
        if let Some(status) = status {
            query = query.filter(Column::Status.eq(status));
        }
        if let Some(job_id) = archive_job_id {
            query = query.filter(Column::ArchiveJobId.eq(job_id));
        }
        let total = query.clone().count(db).await?;
        let items = query
            .order_by_desc(Column::Id)
            .offset(page.saturating_sub(1).saturating_mul(25))
            .limit(25)
            .all(db)
            .await?;
        Ok((items, total))
    }
    pub async fn attach_archive_job(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
        job_id: i32,
    ) -> Result<(), DbErr> {
        Self::update_many()
            .set(ActiveModel {
                archive_job_id: Set(Some(job_id)),
                ..Default::default()
            })
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(import_id))
            .filter(Column::Source.eq("archive_url_import"))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn references_path(db: &impl ConnectionTrait, path: &str) -> Result<bool, DbErr> {
        Ok(Self::find().filter(Column::StoragePath.eq(path)).count(db).await? > 0)
    }
    pub async fn find_owned(db: &impl ConnectionTrait, user_id: i32, import_id: i32) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(import_id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }

    pub async fn delete_by_user(db: &impl ConnectionTrait, import_id: i32, user_id: i32) -> Result<(), DbErr> {
        Self::delete_by_id(import_id)
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn generated_strava_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        after_id: i32,
        limit: u64,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.gt(after_id))
            .filter(Column::Source.eq(STRAVA_SOURCE))
            .filter(Column::Format.eq("tcx"))
            .order_by_asc(Column::Id)
            .limit(limit.clamp(1, 32))
            .all(db)
            .await
    }

    pub async fn archive_sources_for_activity(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.eq(activity_id))
            .filter(Column::Source.eq("archive_url_import"))
            .order_by_asc(Column::Id)
            .limit(32)
            .all(db)
            .await
    }

    pub async fn failed_count<C: ConnectionTrait>(db: &C) -> Result<u64, DbErr> {
        Self::find().filter(Column::Status.eq("failed")).count(db).await
    }
}

impl Model {
    pub fn require_sql_processing(&self) -> Result<(), crate::workflow_error::WorkflowError> {
        if self.source == STRAVA_SOURCE {
            return Err(crate::workflow_error::WorkflowError::bad_request(
                "Use Strava sync or backfill for Strava imports",
            ));
        }
        Ok(())
    }

    pub async fn promote_source(
        &self,
        db: &impl ConnectionTrait,
        artifact: &super::activity_import_artifacts::Model,
    ) -> Result<bool, DbErr> {
        if artifact.user_id != self.user_id || artifact.activity_import_id != self.id {
            return Err(DbErr::Custom("Source artifact ownership mismatch".into()));
        }
        Ok(Entity::update_many()
            .set(ActiveModel {
                import_version: Set(ACTIVITY_IMPORT_VERSION_CURRENT),
                format: Set(artifact.format.clone()),
                storage_path: Set(artifact.storage_path.clone()),
                original_filename: Set(artifact.original_filename.clone()),
                size_bytes: Set(artifact.size_bytes),
                mime_type: Set(artifact.mime_type.clone()),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(self.id))
            .filter(Column::UserId.eq(self.user_id))
            .filter(Column::UpdatedAt.eq(self.updated_at))
            .exec(db)
            .await?
            .rows_affected
            == 1)
    }
}

impl ActiveModel {
    fn with_strava_source(
        self,
        object: &crate::strava::objects::ObjectRef,
        upload: &crate::activity_import_pipeline::ActivityUploadPayload,
    ) -> Self {
        Self {
            import_version: Set(ACTIVITY_IMPORT_VERSION_CURRENT),
            source: Set(STRAVA_SOURCE.into()),
            format: Set(upload.format.clone()),
            status: Set(crate::activity_import_lifecycle::ACTIVITY_IMPORT_STATUS_PROCESSING.into()),
            processing_stage: Set(crate::activity_import_lifecycle::ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED.into()),
            processing_error: Set(None),
            original_filename: Set(upload.original_filename.clone()),
            storage_path: Set(object.path.clone()),
            size_bytes: Set(object.bytes as i64),
            mime_type: Set(upload.mime_type.clone()),
            ..self
        }
    }
}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Entity {
    pub async fn by_ids(
        db: &impl ConnectionTrait,
        import_ids: impl IntoIterator<Item = i32>,
    ) -> Result<Vec<Model>, DbErr> {
        Self::find().filter(Column::Id.is_in(import_ids)).all(db).await
    }
    pub async fn resumable_for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Model>, DbErr> {
        Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.ne(STRAVA_SOURCE))
            .filter(Column::Status.is_in(["processing", "failed"]))
            .order_by_asc(Column::CreatedAt)
            .all(db)
            .await
    }
    pub async fn stale_manual_owner_ids(
        db: &impl ConnectionTrait,
        stale_before: DateTime<Utc>,
    ) -> Result<Vec<i32>, DbErr> {
        Self::find()
            .select_only()
            .column(Column::UserId)
            .distinct()
            .filter(Column::Source.eq("manual_upload"))
            .filter(Column::Status.eq("processing"))
            .filter(Column::LastProcessingEventAt.lte(stale_before))
            .into_tuple()
            .all(db)
            .await
    }
    pub async fn processing_owner_ids(db: &impl ConnectionTrait, source: Option<&str>) -> Result<Vec<i32>, DbErr> {
        let mut query = Self::find()
            .select_only()
            .column(Column::UserId)
            .distinct()
            .filter(Column::Status.eq("processing"))
            .filter(Column::Source.ne(STRAVA_SOURCE));
        if let Some(source) = source {
            query = query.filter(Column::Source.eq(source));
        }
        query.into_tuple().all(db).await
    }
    pub async fn archive_statuses(db: &impl ConnectionTrait, job_id: i32) -> Result<Vec<String>, DbErr> {
        Self::find()
            .filter(Column::ArchiveJobId.eq(job_id))
            .select_only()
            .column(Column::Status)
            .into_tuple()
            .all(db)
            .await
    }
}

mod mutations;
