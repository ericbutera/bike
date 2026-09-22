use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, QueryOrder, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "segment_efforts")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub segment_id: i32,
    pub activity_id: i32,
    pub effort_index: i32,
    pub start_route_point_index: i32,
    pub end_route_point_index: i32,
    pub start_elapsed_seconds: i32,
    pub end_elapsed_seconds: i32,
    pub duration_seconds: i32,
    pub distance_meters: Option<f64>,
    pub overall_rank: Option<i32>,
    pub user_rank: Option<i32>,
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
    pub async fn list_for_segment_analytics<C>(
        db: &C,
        segment_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .order_by_asc(Column::SegmentId)
            .order_by_asc(Column::DurationSeconds)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    pub async fn list_for_activity_analytics<C>(
        db: &C,
        activity_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .order_by_asc(Column::ActivityId)
            .order_by_asc(Column::DurationSeconds)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    pub async fn list_by_user_activity_ids<C>(
        db: &C,
        user_id: i32,
        activity_ids: &[i32],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .order_by_asc(Column::ActivityId)
            .order_by_asc(Column::StartRoutePointIndex)
            .order_by_asc(Column::EndRoutePointIndex)
            .order_by_asc(Column::DurationSeconds)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    pub async fn list_by_segment_ids<C>(db: &C, segment_ids: &[i32]) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
    }
}
