use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter, QueryOrder, Set};

use super::activity_imports;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_import_artifacts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub activity_import_id: i32,
    pub user_id: i32,
    pub artifact_kind: String,
    pub format: String,
    pub source_quality: String,
    pub original_filename: String,
    pub storage_path: String,
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    pub checksum_sha256: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn find_existing_original_by_checksum<C>(
        db: &C,
        user_id: i32,
        checksum: &str,
    ) -> Result<Option<activity_imports::Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        let artifact = Self::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ArtifactKind.eq("original"))
            .filter(Column::ChecksumSha256.eq(checksum))
            .order_by_desc(Column::Id)
            .one(db)
            .await?;

        let Some(artifact) = artifact else {
            return Ok(None);
        };

        activity_imports::Entity::find_by_id(artifact.activity_import_id)
            .filter(activity_imports::Column::UserId.eq(user_id))
            .one(db)
            .await
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);
        Ok(self)
    }
}
