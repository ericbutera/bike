use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};

impl Model {
    pub async fn begin_disconnect(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
    ) -> Result<(), DbErr> {
        let connection = Self::lock_event(db, event).await?;
        let requested = connection.disconnect_requested_at.unwrap_or_else(Utc::now);
        let mut active: ActiveModel = connection.into();
        active.disconnect_requested_at = Set(Some(requested));
        active.update(db).await?;
        Ok(())
    }
    pub async fn complete_native_sync(
        db: &impl ConnectionTrait,
        result: &crate::strava::native::CollectionResult,
    ) -> Result<(), DbErr> {
        let connection = Self::lock_event(db, &result.event).await?;
        let cursor = connection.last_synced_activity_started_at;
        let mut active: ActiveModel = connection.into();
        if result.event.operation == crate::strava::events::StravaOperation::Sync {
            active.last_synced_activity_started_at =
                Set(cursor.into_iter().chain(result.last_started_at).max());
        }
        active.last_sync_status = Set("succeeded".into());
        active.last_sync_finished_at = Set(Some(Utc::now()));
        active.last_sync_imported_count = Set(result.imported);
        active.last_sync_duplicate_count = Set(result.unchanged);
        active.last_sync_failed_count = Set(0);
        active.last_sync_message = Set(None);
        active.update(db).await?;
        Ok(())
    }

    pub async fn remove_native(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
    ) -> Result<(), DbErr> {
        let Some(connection) = Self::load(db, event.connection_id).await? else {
            return Ok(());
        };
        event
            .validate_connection(&connection)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        Entity::delete_many()
            .filter(Column::Id.eq(event.connection_id))
            .filter(Column::CreatedAt.eq(event.connection_generation))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn load(db: &impl ConnectionTrait, id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }

    pub async fn for_event(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
    ) -> Result<Self, crate::workflow_error::WorkflowError> {
        let connection = Self::load(db, event.connection_id).await?.ok_or_else(|| {
            crate::workflow_error::WorkflowError::not_found("Strava connection no longer exists")
        })?;
        event.validate_connection(&connection)?;
        Ok(connection)
    }

    pub async fn lock_event(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
    ) -> Result<Self, DbErr> {
        use sea_orm::QuerySelect;
        let connection = Entity::find_by_id(event.connection_id)
            .lock_exclusive()
            .one(db)
            .await?
            .ok_or(DbErr::RecordNotFound(
                "Strava connection was removed".into(),
            ))?;
        event
            .validate_connection(&connection)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        Ok(connection)
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
    pub async fn scan_page(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<Self>, DbErr> {
        use sea_orm::{QueryOrder, QuerySelect};
        Entity::find()
            .filter(Column::Id.gt(after_id))
            .filter(Column::DisconnectRequestedAt.is_null())
            .order_by_asc(Column::Id)
            .limit(20)
            .all(db)
            .await
    }
}

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_connections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub disconnect_requested_at: Option<DateTime<Utc>>,
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
