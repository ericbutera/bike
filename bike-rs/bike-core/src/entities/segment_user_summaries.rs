use crate::analytics::SegmentUserSummaryAccumulator;
use crate::entities::segment_user_summaries;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "segment_user_summaries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub segment_id: i32,
    pub user_id: i32,
    #[sea_orm(default_value = 0)]
    pub effort_count: i32,
    pub personal_best_effort_id: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    pub async fn list_by_user_segment_ids<C>(db: &C, user_id: i32, segment_ids: &[i32]) -> Result<Vec<Model>, DbErr>
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

impl Entity {
    pub async fn delete_for_segment(db: &impl ConnectionTrait, segment_id: i32) -> Result<(), DbErr> {
        Self::delete_many()
            .filter(Column::SegmentId.eq(segment_id))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl Entity {
    pub async fn clear_segments(db: &impl ConnectionTrait, ids: &[i32]) -> Result<sea_orm::DeleteResult, DbErr> {
        use sea_orm::QueryFilter;
        Self::delete_many()
            .filter(Column::SegmentId.is_in(ids.iter().copied()))
            .exec(db)
            .await
    }
}

impl Model {
    pub(crate) async fn store_summary(
        db: &impl ConnectionTrait,
        segment_id: i32,
        user_id: i32,
        summary: SegmentUserSummaryAccumulator,
    ) -> Result<(), DbErr> {
        segment_user_summaries::ActiveModel {
            segment_id: Set(segment_id),
            user_id: Set(user_id),
            effort_count: Set(summary.effort_count),
            personal_best_effort_id: Set(summary.personal_best_effort_id),
            personal_best_duration_seconds: Set(summary.personal_best_duration_seconds),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }
}
