use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, QueryFilter, QueryOrder, QuerySelect, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub async fn ranked_for_user_segment(
        db: &impl ConnectionTrait,
        user_id: i32,
        segment_id: i32,
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::SegmentId.eq(segment_id))
            .filter(Column::UserId.eq(user_id))
            .order_by_asc(Column::DurationSeconds)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    pub async fn ranked_for_segments(
        db: &impl ConnectionTrait,
        segment_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .order_by_asc(Column::DurationSeconds)
            .order_by_asc(Column::Id)
            .all(db)
            .await
    }

    pub async fn count_for_activity(
        db: &impl ConnectionTrait,
        activity_id: i32,
    ) -> Result<u64, DbErr> {
        use sea_orm::PaginatorTrait;
        Entity::find()
            .filter(Column::ActivityId.eq(activity_id))
            .count(db)
            .await
    }

    pub async fn activity_ids_for_segments(
        db: &impl ConnectionTrait,
        segment_ids: &[i32],
    ) -> Result<Vec<i32>, DbErr> {
        let mut ids = Entity::find()
            .select_only()
            .column(Column::ActivityId)
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .into_tuple::<i32>()
            .all(db)
            .await?;
        ids.sort_unstable();
        ids.dedup();
        Ok(ids)
    }

    pub async fn activity_ids_for_segment<C>(db: &C, segment_id: i32) -> Result<Vec<i32>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .select_only()
            .column(Column::ActivityId)
            .distinct()
            .filter(Column::SegmentId.eq(segment_id))
            .into_tuple::<i32>()
            .all(db)
            .await
    }

    pub async fn segment_ids_for_activity<C>(db: &C, activity_id: i32) -> Result<Vec<i32>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .select_only()
            .column(Column::SegmentId)
            .distinct()
            .filter(Column::ActivityId.eq(activity_id))
            .into_tuple::<i32>()
            .all(db)
            .await
    }

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

impl Entity {
    pub async fn delete_for_segment(
        db: &impl ConnectionTrait,
        segment_id: i32,
    ) -> Result<(), DbErr> {
        Self::delete_many()
            .filter(Column::SegmentId.eq(segment_id))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn delete_for_owned_activity(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_id: i32,
    ) -> Result<(), DbErr> {
        Self::delete_many()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::ActivityId.eq(activity_id))
            .exec(db)
            .await?;
        Ok(())
    }

    pub(crate) async fn store_matches(
        db: &impl ConnectionTrait,
        user_id: i32,
        segment_id: i32,
        activity_id: i32,
        matches: &[crate::segment_support::MatchedSegmentEffort],
    ) -> Result<(), DbErr> {
        if matches.is_empty() {
            return Ok(());
        }
        let efforts = matches
            .iter()
            .enumerate()
            .map(|(index, matched)| ActiveModel {
                user_id: Set(user_id),
                segment_id: Set(segment_id),
                activity_id: Set(activity_id),
                effort_index: Set((index + 1) as i32),
                start_route_point_index: Set(matched.start_route_point_index),
                end_route_point_index: Set(matched.end_route_point_index),
                start_elapsed_seconds: Set(matched.start_elapsed_seconds),
                end_elapsed_seconds: Set(matched.end_elapsed_seconds),
                duration_seconds: Set(matched.duration_seconds),
                distance_meters: Set(matched.distance_meters),
                ..Default::default()
            });
        Self::insert_many(efforts).exec(db).await?;
        Ok(())
    }
}

impl Model {
    pub async fn for_owned_segments(
        db: &impl ConnectionTrait,
        user_id: i32,
        segment_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
    }
    pub async fn for_activity_ids(
        db: &impl ConnectionTrait,
        activity_ids: &[i32],
    ) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::ActivityId.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }
}

impl Model {
    pub async fn set_ranks(
        self,
        db: &impl ConnectionTrait,
        overall_rank: i32,
        user_rank: i32,
    ) -> Result<Self, DbErr> {
        let mut active_model: ActiveModel = self.into();
        active_model.overall_rank = Set(Some(overall_rank));
        active_model.user_rank = Set(Some(user_rank));
        active_model.update(db).await
    }
}
