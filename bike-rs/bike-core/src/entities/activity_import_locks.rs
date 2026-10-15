use crate::activity_import_lock::*;
use crate::entities::activity_import_locks;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::DatabaseConnection;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_import_locks")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub source: String,
    pub stage: String,
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

impl Model {
    pub async fn for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Option<Self>, DbErr> {
        use sea_orm::QueryFilter;
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }
    pub async fn release(self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        Entity::delete_by_id(self.id).exec(db).await?;
        Ok(())
    }
}

impl Model {
    pub async fn release_batch_lock(
        db: &impl ConnectionTrait,
        id: Option<i32>,
        user_id: i32,
        source: &str,
    ) -> Result<(), DbErr> {
        use sea_orm::QueryFilter;
        Entity::delete_many()
            .filter(Column::Id.eq(id))
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Source.eq(source))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl Model {
    pub async fn acquire(
        db: &DatabaseConnection,
        user_id: i32,
        source: &str,
        stage: &str,
    ) -> Result<activity_import_locks::Model, ActivityImportLockError> {
        let lock = activity_import_locks::ActiveModel {
            user_id: Set(user_id),
            source: Set(source.to_string()),
            stage: Set(stage.to_string()),
            ..Default::default()
        }
        .insert(db)
        .await;

        match lock {
            Ok(lock) => Ok(lock),
            Err(error) => {
                if let Some(existing) = Self::for_user(db, user_id).await? {
                    return Err(ActivityImportLockError::conflict(format!(
                        "Another activity processing operation is already {} for this user ({})",
                        describe_stage(&existing.stage),
                        describe_source(&existing.source),
                    )));
                }

                Err(ActivityImportLockError::from(error))
            }
        }
    }
}

impl Model {
    pub async fn mark_stage(
        db: &DatabaseConnection,
        user_id: i32,
        source: &str,
        stage: &str,
    ) -> Result<activity_import_locks::Model, ActivityImportLockError> {
        let lock = Self::for_user(db, user_id).await?.ok_or_else(|| {
            ActivityImportLockError::internal(format!(
                "Activity import lock for user {user_id} was missing while starting {}",
                describe_source(source),
            ))
        })?;

        if lock.source != source {
            return Err(ActivityImportLockError::internal(format!(
                "Activity import lock for user {} is owned by {} instead of {}",
                user_id,
                describe_source(&lock.source),
                describe_source(source),
            )));
        }

        let mut active_model: activity_import_locks::ActiveModel = lock.into();
        active_model.stage = Set(stage.to_string());
        active_model
            .update(db)
            .await
            .map_err(ActivityImportLockError::from)
    }
}

impl Model {
    pub async fn ensure_stage(
        db: &DatabaseConnection,
        user_id: i32,
        source: &str,
        stage: &str,
    ) -> Result<activity_import_locks::Model, ActivityImportLockError> {
        let Some(lock) = Self::for_user(db, user_id).await? else {
            tracing::warn!(
                user_id,
                source,
                stage,
                "activity import lock was missing; reacquiring lock for queued work"
            );
            return Self::acquire(db, user_id, source, stage).await;
        };

        if lock.source != source {
            return Err(ActivityImportLockError::internal(format!(
                "Activity import lock for user {} is owned by {} instead of {}",
                user_id,
                describe_source(&lock.source),
                describe_source(source),
            )));
        }

        let mut active_model: activity_import_locks::ActiveModel = lock.into();
        active_model.stage = Set(stage.to_string());
        active_model
            .update(db)
            .await
            .map_err(ActivityImportLockError::from)
    }
}
