use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "segment_user_summaries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub segment_id: i32,
    pub user_id: i32,
    pub effort_count: i32,
    pub personal_best_effort_id: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
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
    pub async fn list_by_user_segment_ids<C>(
        db: &C,
        user_id: i32,
        segment_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
    }

    pub async fn list_by_segment_and_user_ids<C>(
        db: &C,
        segment_ids: &[i32],
        user_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() || user_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .filter(Column::UserId.is_in(user_ids.iter().copied()))
            .all(db)
            .await
    }
}
