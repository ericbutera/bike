use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

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
    pub total_entries: i32,
    pub supported_entry_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub skipped_unsupported_count: i32,
    pub failed_count: i32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn store_progress(
        db: &impl ConnectionTrait,
        user_id: i32,
        job_id: i32,
        summary: &crate::archive_import::ActivityArchiveImportResponse,
    ) -> Result<(), DbErr> {
        let errors = serde_json::to_string(&summary.error_samples)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        let result = Self::update_many()
            .set(ActiveModel {
                total_entries: Set(summary.total_entries),
                supported_entry_count: Set(summary.supported_entry_count),
                imported_count: Set(summary.imported_count),
                duplicate_count: Set(summary.duplicate_count),
                skipped_unsupported_count: Set(summary.skipped_unsupported_count),
                failed_count: Set(summary.failed_count),
                error_samples_json: Set(Some(errors)),
                updated_at: Set(Utc::now()),
                ..Default::default()
            })
            .filter(Column::Id.eq(job_id))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        if result.rows_affected != 1 {
            return Err(DbErr::RecordNotFound(
                "Owned archive job was not found".into(),
            ));
        }
        Ok(())
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
