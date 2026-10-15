use crate::activity_achievements::StoredActivityAchievementHighlights;
use crate::analytics::ActivityAnalyticsAccumulator;
use crate::entities::activity_analytics;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_analytics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    #[sea_orm(default_value = 0)]
    pub segment_effort_count: i32,
    #[sea_orm(default_value = 0)]
    pub achievement_count: i32,
    #[sea_orm(default_value = 0)]
    pub kom_count: i32,
    #[sea_orm(default_value = 0)]
    pub top_10_count: i32,
    #[sea_orm(default_value = 0)]
    pub pr_count: i32,
    pub achievement_highlights_json: Option<StoredActivityAchievementHighlights>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl Entity {
    pub async fn delete_by_activity_id(
        db: &impl ConnectionTrait,
        activity_id: i32,
    ) -> Result<(), DbErr> {
        Self::delete_by_id(activity_id).exec(db).await?;
        Ok(())
    }
}

crate::timestamps::impl_timestamps!(ActiveModel);

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

impl Model {
    pub(crate) async fn store_summary(
        db: &impl ConnectionTrait,
        activity_id: i32,
        user_id: i32,
        analytics: ActivityAnalyticsAccumulator,
    ) -> Result<(), DbErr> {
        activity_analytics::ActiveModel {
            activity_id: Set(activity_id),
            user_id: Set(user_id),
            segment_effort_count: Set(analytics.segment_effort_count),
            achievement_count: Set(analytics.achievement_count),
            kom_count: Set(analytics.kom_count),
            top_10_count: Set(analytics.top_10_count),
            pr_count: Set(analytics.pr_count),
            achievement_highlights_json: Set(Some(
                StoredActivityAchievementHighlights::from_items(analytics.achievement_highlights),
            )),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }
}
