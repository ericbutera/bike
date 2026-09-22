use crate::activity_achievements::StoredActivityAchievementHighlights;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_analytics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    pub segment_effort_count: i32,
    pub achievement_count: i32,
    pub kom_count: i32,
    pub top_10_count: i32,
    pub pr_count: i32,
    pub achievement_highlights_json: Option<StoredActivityAchievementHighlights>,
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
    pub async fn delete_by_activity_ids<C>(db: &C, activity_ids: &[i32]) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        if activity_ids.is_empty() {
            return Ok(());
        }

        Entity::delete_many()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .exec(db)
            .await?;

        Ok(())
    }

    pub async fn list_by_user_activity_ids<C>(
        db: &C,
        user_id: i32,
        activity_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if activity_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }
}
