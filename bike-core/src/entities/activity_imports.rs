use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ActiveValue, ConnectionTrait, DbErr, Set};

pub const ACTIVITY_IMPORT_VERSION_LEGACY: i32 = 1;
pub const ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE: i32 = 2;
pub const ACTIVITY_IMPORT_VERSION_CURRENT: i32 = ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
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
