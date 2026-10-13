use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbErr, QueryFilter, Set,
    TransactionTrait,
};

impl Model {
    /// Provider execution and refresh share a transaction-scoped connection lease.
    pub async fn execution_lease(
        db: &DatabaseConnection,
        id: i32,
    ) -> Result<Option<DatabaseTransaction>, DbErr> {
        if db.get_database_backend() != sea_orm::DatabaseBackend::Postgres {
            return Ok(None);
        }
        let lease = db.begin().await?;
        // PostgreSQL advisory locks need the database's native lock primitive.
        lease
            .execute_unprepared("SELECT pg_advisory_xact_lock_shared(7393, 0)")
            .await?;
        lease
            .execute_raw(sea_orm::Statement::from_sql_and_values(
                sea_orm::DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(7391, $1)",
                [id.into()],
            ))
            .await?;
        Ok(Some(lease))
    }

    pub async fn for_athlete(
        db: &impl ConnectionTrait,
        athlete_id: i64,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::AthleteId.eq(athlete_id))
            .one(db)
            .await
    }
    pub async fn all_connected(db: &impl ConnectionTrait) -> Result<Vec<Self>, DbErr> {
        Entity::find().all(db).await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_connections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub athlete_id: i64,
    pub athlete_username: Option<String>,
    pub athlete_first_name: Option<String>,
    pub athlete_last_name: Option<String>,
    pub athlete_profile_medium_url: Option<String>,
    pub scopes: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
    pub last_synced_activity_started_at: Option<DateTime<Utc>>,
    pub last_sync_status: String,
    pub last_sync_message: Option<String>,
    pub last_sync_started_at: Option<DateTime<Utc>>,
    pub last_sync_finished_at: Option<DateTime<Utc>>,
    pub last_sync_imported_count: i32,
    pub last_sync_duplicate_count: i32,
    pub last_sync_failed_count: i32,
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
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);
        Ok(self)
    }
}
