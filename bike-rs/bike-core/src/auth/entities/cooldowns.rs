use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "cooldowns")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub subject_type: String,
    pub subject_id: Option<i32>,
    pub action: String,
    pub last_run: Option<DateTime<Utc>>,
    pub attempt_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

use async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait,
    IntoActiveModel, QueryFilter, Set,
};

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
    pub async fn find_by(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<Option<Self>, DbErr> {
        Entity::find()
            .filter(Column::SubjectType.eq(subject_type))
            .filter(Column::SubjectId.eq(subject_id))
            .filter(Column::Action.eq(action))
            .one(db)
            .await
    }

    async fn find_or_new(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<ActiveModel, DbErr> {
        let existing = Self::find_by(db, subject_type, subject_id, action).await?;

        Ok(match existing {
            Some(model) => model.into_active_model(),
            None => ActiveModel {
                subject_type: Set(subject_type.to_owned()),
                subject_id: Set(subject_id),
                action: Set(action.to_owned()),
                attempt_count: Set(0),
                ..Default::default()
            },
        })
    }

    pub async fn increment_attempts(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<i32, DbErr> {
        let mut active = Self::find_or_new(db, subject_type, subject_id, action).await?;

        let new_count = active.attempt_count.as_ref() + 1;

        active.attempt_count = Set(new_count);
        active.last_run = Set(Some(Utc::now()));

        active.save(db).await?;

        Ok(new_count)
    }

    pub async fn upsert_last_run(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
        last_run: DateTime<Utc>,
    ) -> Result<(), DbErr> {
        let mut active = Self::find_or_new(db, subject_type, subject_id, action).await?;

        active.last_run = Set(Some(last_run));
        active.save(db).await?;

        Ok(())
    }

    pub async fn reset_attempts(
        db: &DatabaseConnection,
        subject_type: &str,
        subject_id: Option<i32>,
        action: &str,
    ) -> Result<(), DbErr> {
        if let Some(rec) = Entity::find()
            .filter(Column::SubjectType.eq(subject_type))
            .filter(Column::SubjectId.eq(subject_id))
            .filter(Column::Action.eq(action))
            .one(db)
            .await?
        {
            let mut active = rec.into_active_model();
            active.attempt_count = Set(0);
            active.last_run = Set(None);
            active.update(db).await?;
        }
        Ok(())
    }
}
