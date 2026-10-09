use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, ConnectionTrait, DbErr, QueryFilter, QueryOrder, QuerySelect, Set};

pub const ACTIVITY_IMPORT_VERSION_LEGACY: i32 = 1;
pub const ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE: i32 = 2;
pub const ACTIVITY_IMPORT_VERSION_CURRENT: i32 = ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "activity_imports")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub import_version: i32,
    pub source: String,
    pub format: String,
    pub status: String,
    pub activity_id: Option<i32>,
    pub processing_stage: String,
    pub processing_error: Option<String>,
    pub processing_attempts: i32,
    pub processed_at: Option<DateTime<Utc>>,
    pub last_processing_event_at: Option<DateTime<Utc>>,
    pub original_filename: String,
    pub storage_path: String,
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn mark_recovery_pending(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<(), DbErr> {
        Self::update_many()
            .set(ActiveModel {
                status: Set("uploaded".into()),
                processing_stage: Set("source_recovery".into()),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Id.eq(import_id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn references_path(db: &impl ConnectionTrait, path: &str) -> Result<bool, DbErr> {
        Ok(Self::find()
            .filter(Column::StoragePath.eq(path))
            .count(db)
            .await?
            > 0)
    }
    pub async fn find_owned(
        db: &impl ConnectionTrait,
        user_id: i32,
        import_id: i32,
    ) -> Result<Option<Model>, DbErr> {
        Self::find_by_id(import_id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
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
            .filter(Column::Source.eq("strava_sync"))
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
        Self::find()
            .filter(Column::Status.eq("failed"))
            .count(db)
            .await
    }
}

impl Model {
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

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();
        if matches!(self.import_version, ActiveValue::NotSet) {
            self.import_version = Set(ACTIVITY_IMPORT_VERSION_CURRENT);
        }
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);
        Ok(self)
    }
}
