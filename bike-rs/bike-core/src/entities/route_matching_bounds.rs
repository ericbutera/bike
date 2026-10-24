//! Saved matching bounds, independent of heatmap eligibility.
use crate::{
    activity_data::ActivityRoutePoint,
    bike_tasks::{ActivityTask, Priority},
    object_store::ObjectStore,
    workflow_error::WorkflowError,
};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{extension::postgres::PgBinOper, Alias, Expr, Func, OnConflict, SelectStatement, UnionType};
use sea_orm::{
    ExprTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select, Set, TransactionTrait,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub min_latitude: f64,
    pub max_latitude: f64,
    pub min_longitude: f64,
    pub max_longitude: f64,
}

impl Bounds {
    pub fn from_route(route: &[ActivityRoutePoint]) -> Result<Option<Self>, DbErr> {
        if route.len() < 2 {
            return Ok(None);
        }
        let mut bounds = Self {
            min_latitude: 90.0,
            max_latitude: -90.0,
            min_longitude: 180.0,
            max_longitude: -180.0,
        };
        for point in route {
            if !point.latitude.is_finite() || !point.longitude.is_finite() {
                return Err(DbErr::Custom("Invalid matching route coordinate".into()));
            }
            if !(-90.0..=90.0).contains(&point.latitude) || !(-180.0..=180.0).contains(&point.longitude) {
                return Err(DbErr::Custom("Matching route coordinate outside Earth".into()));
            }
            bounds.min_latitude = bounds.min_latitude.min(point.latitude);
            bounds.max_latitude = bounds.max_latitude.max(point.latitude);
            bounds.min_longitude = bounds.min_longitude.min(point.longitude);
            bounds.max_longitude = bounds.max_longitude.max(point.longitude);
        }
        if bounds.max_longitude - bounds.min_longitude > 180.0 {
            bounds.min_longitude = -180.0;
            bounds.max_longitude = 180.0;
        }
        Ok(Some(bounds))
    }

    pub(super) fn overlaps(self, minimum: [Expr; 2], maximum: [Expr; 2]) -> Expr {
        let corridor = self.matching_corridor();
        postgres_box(minimum, maximum).binary(
            PgBinOper::Overlap,
            postgres_box(
                [Expr::val(corridor.min_longitude), Expr::val(corridor.min_latitude)],
                [Expr::val(corridor.max_longitude), Expr::val(corridor.max_latitude)],
            ),
        )
    }

    pub fn matching_corridor(self) -> Self {
        // The additional meter covers source/recording coordinate quantization.
        let angle = (crate::segment_support::candidate_corridor_meters() + 1.0) / 6_371_000.0;
        let latitude_margin = angle.to_degrees();
        let min_latitude = (self.min_latitude - latitude_margin).max(-90.0);
        let max_latitude = (self.max_latitude + latitude_margin).min(90.0);
        let polar = min_latitude <= -90.0 || max_latitude >= 90.0;
        let extreme = min_latitude.abs().max(max_latitude.abs()).to_radians();
        let ratio = angle.sin() / extreme.cos();
        let longitude_margin = ratio.clamp(0.0, 1.0).asin().to_degrees();
        let wraps = self.min_longitude - longitude_margin < -180.0 || self.max_longitude + longitude_margin > 180.0;
        let (min_longitude, max_longitude) = if polar || wraps {
            (-180.0, 180.0)
        } else {
            (
                self.min_longitude - longitude_margin,
                self.max_longitude + longitude_margin,
            )
        };
        Self {
            min_latitude,
            max_latitude,
            min_longitude,
            max_longitude,
        }
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_route_bounds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub point_count: i64,
    pub min_latitude: Option<f64>,
    pub max_latitude: Option<f64>,
    pub min_longitude: Option<f64>,
    pub max_longitude: Option<f64>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
#[derive(FromQueryResult)]
struct PendingActivity {
    activity_id: i32,
    user_id: i32,
}
impl Model {
    /// The caller inserted or locked this activity in its transaction.
    pub async fn store_source(db: &impl ConnectionTrait, activity: &super::activities::Model) -> Result<(), DbErr> {
        let data = crate::activity_data::deserialize_derived_activity_data(activity.derived_data_json.as_ref());
        Self::store_route(db, activity.id, &data.route_points).await
    }

    async fn store_route(
        db: &impl ConnectionTrait,
        activity_id: i32,
        route: &[ActivityRoutePoint],
    ) -> Result<(), DbErr> {
        let bounds = Bounds::from_route(route)?;
        Entity::insert(ActiveModel {
            activity_id: Set(activity_id),
            point_count: Set(
                i64::try_from(route.len()).map_err(|_| DbErr::Custom("Matching route is too large".into()))?
            ),
            min_latitude: Set(bounds.map(|bounds| bounds.min_latitude)),
            max_latitude: Set(bounds.map(|bounds| bounds.max_latitude)),
            min_longitude: Set(bounds.map(|bounds| bounds.min_longitude)),
            max_longitude: Set(bounds.map(|bounds| bounds.max_longitude)),
        })
        .on_conflict(
            OnConflict::column(Column::ActivityId)
                .update_columns([
                    Column::PointCount,
                    Column::MinLatitude,
                    Column::MaxLatitude,
                    Column::MinLongitude,
                    Column::MaxLongitude,
                ])
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }

    pub async fn pending_activities(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<ActivityTask>, DbErr> {
        use super::activities;

        let rows = Self::activities_without_bounds()
            .select_only()
            .column_as(activities::Column::Id, "activity_id")
            .column(activities::Column::UserId)
            .filter(activities::Column::Id.gt(after_id))
            .order_by_asc(activities::Column::Id)
            .limit(16)
            .into_model::<PendingActivity>()
            .all(db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|row| ActivityTask {
                activity_id: row.activity_id,
                user_id: row.user_id,
                priority: Priority::Background,
            })
            .collect())
    }

    pub async fn pending_segments(db: &impl ConnectionTrait, after_id: i32) -> Result<Vec<i32>, DbErr> {
        super::segments::Model::pending_matching_bounds_ids(db, after_id).await
    }

    pub async fn backfill_activity(
        db: &DatabaseConnection,
        objects: &ObjectStore,
        task: ActivityTask,
    ) -> Result<(), WorkflowError> {
        let txn = db.begin().await?;
        if super::activities::Model::lock_owned(&txn, task.activity_id, task.user_id)
            .await?
            .is_some()
        {
            let recording = super::activity_recordings::Model::load(&txn, objects, task.activity_id, task.user_id)
                .await?
                .ok_or_else(|| {
                    WorkflowError::invalid_data(
                        "Normalized recording is missing; run backfill_activity_recordings first",
                    )
                })?;
            Self::store_route(&txn, task.activity_id, &recording.route_points).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    pub async fn backfill_segment(db: &DatabaseConnection, segment_id: i32) -> Result<(), WorkflowError> {
        let txn = db.begin().await?;
        if let Some(segment) = super::segments::Model::lock_for_analytics(&txn, segment_id).await? {
            let route = crate::segment_support::deserialize_segment_route_points(segment.route_data_json.as_ref());
            segment::Model::store_route(&txn, segment.id, &route).await?;
        }
        txn.commit().await?;
        Ok(())
    }

    /// Background matching across riders: only eligible activity IDs, never recordings.
    pub async fn matching_activity_ids(
        db: &impl ConnectionTrait,
        bounds: Option<Bounds>,
        after_id: i32,
        limit: i64,
    ) -> Result<Vec<i32>, DbErr> {
        use super::activities;

        activities::Entity::find()
            .select_only()
            .column(activities::Column::Id)
            .filter(activities::Column::Sport.is_in(activities::BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .filter(activities::Column::Id.gt(after_id))
            .apply_if(bounds, |query, bounds| {
                query.filter(activities::Column::Id.in_subquery(Self::candidate_activity_ids(bounds)))
            })
            .order_by_asc(activities::Column::Id)
            .limit(limit.clamp(1, 16) as u64)
            .into_tuple::<i32>()
            .all(db)
            .await
    }

    fn candidate_activity_ids(bounds: Bounds) -> SelectStatement {
        Self::overlapping_activity_ids(bounds)
            .union(
                UnionType::All,
                Self::activities_without_bounds()
                    .select_only()
                    .column(super::activities::Column::Id)
                    .into_query(),
            )
            .to_owned()
    }

    fn overlapping_activity_ids(bounds: Bounds) -> SelectStatement {
        Entity::find()
            .select_only()
            .column(Column::ActivityId)
            // Match the partial GiST index predicate even with a generic prepared plan.
            .filter(Column::PointCount.into_expr().gte(Expr::Constant(2.into())))
            .filter(bounds.overlaps(
                [Column::MinLongitude.into_expr(), Column::MinLatitude.into_expr()],
                [Column::MaxLongitude.into_expr(), Column::MaxLatitude.into_expr()],
            ))
            .into_query()
    }

    fn activities_without_bounds() -> Select<super::activities::Entity> {
        use super::activities;

        let saved_bounds = Entity::find()
            .select_only()
            .column(Column::ActivityId)
            .filter(Expr::col((Entity, Column::ActivityId)).equals((activities::Entity, activities::Column::Id)))
            .into_query();
        activities::Entity::find().filter(Expr::exists(saved_bounds).not())
    }
}

pub mod segment {
    use super::*;
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "segment_route_bounds")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub segment_id: i32,
        pub point_count: i64,
        pub min_latitude: Option<f64>,
        pub max_latitude: Option<f64>,
        pub min_longitude: Option<f64>,
        pub max_longitude: Option<f64>,
    }
    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
    impl ActiveModelBehavior for ActiveModel {}
    impl Model {
        pub async fn store_route(
            db: &impl ConnectionTrait,
            segment_id: i32,
            route: &[ActivityRoutePoint],
        ) -> Result<(), DbErr> {
            let bounds = Bounds::from_route(route)?;
            Entity::insert(ActiveModel {
                segment_id: Set(segment_id),
                point_count: Set(
                    i64::try_from(route.len()).map_err(|_| DbErr::Custom("Matching route is too large".into()))?
                ),
                min_latitude: Set(bounds.map(|bounds| bounds.min_latitude)),
                max_latitude: Set(bounds.map(|bounds| bounds.max_latitude)),
                min_longitude: Set(bounds.map(|bounds| bounds.min_longitude)),
                max_longitude: Set(bounds.map(|bounds| bounds.max_longitude)),
            })
            .on_conflict(
                OnConflict::column(Column::SegmentId)
                    .update_columns([
                        Column::PointCount,
                        Column::MinLatitude,
                        Column::MaxLatitude,
                        Column::MinLongitude,
                        Column::MaxLongitude,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
            Ok(())
        }
    }
}

fn postgres_box(minimum: [Expr; 2], maximum: [Expr; 2]) -> Expr {
    let minimum = Func::cust(Alias::new("point")).args(minimum);
    let maximum = Func::cust(Alias::new("point")).args(maximum);
    Func::cust(Alias::new("box"))
        .args([Expr::from(minimum), Expr::from(maximum)])
        .into()
}

#[cfg(test)]
mod tests;
