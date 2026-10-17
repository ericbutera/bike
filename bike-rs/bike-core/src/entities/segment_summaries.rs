use crate::analytics::SegmentSummaryAccumulator;
use crate::entities::segment_summaries;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "segment_summaries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub segment_id: i32,
    #[sea_orm(default_value = 0)]
    pub effort_count: i32,
    pub leader_user_id: Option<i32>,
    pub leader_effort_id: Option<i32>,
    pub best_duration_seconds: Option<i32>,
    pub latest_activity_started_at: Option<DateTime<Utc>>,
    pub latest_activity_id: Option<i32>,
    pub latest_effort_id: Option<i32>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Entity {
    pub async fn delete_for_segment(db: &impl ConnectionTrait, segment_id: i32) -> Result<(), DbErr> {
        use sea_orm::QueryFilter;
        Self::delete_many()
            .filter(Column::SegmentId.eq(segment_id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn for_segments(db: &impl ConnectionTrait, segment_ids: &[i32]) -> Result<Vec<Model>, DbErr> {
        use sea_orm::QueryFilter;
        Self::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
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
        summary: SegmentSummaryAccumulator,
    ) -> Result<(), DbErr> {
        segment_summaries::ActiveModel {
            segment_id: Set(segment_id),
            effort_count: Set(summary.effort_count),
            leader_user_id: Set(summary.leader_user_id),
            leader_effort_id: Set(summary.leader_effort_id),
            best_duration_seconds: Set(summary.best_duration_seconds),
            latest_activity_started_at: Set(summary.latest_activity_started_at),
            latest_activity_id: Set(summary.latest_activity_id),
            latest_effort_id: Set(summary.latest_effort_id),
            ..Default::default()
        }
        .insert(db)
        .await?;
        Ok(())
    }
}
