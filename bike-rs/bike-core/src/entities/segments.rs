use crate::activity_data::StoredRoutePointSeries;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::{Expr, SelectStatement, UnionType};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, ExprTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, QueryTrait,
    Select, TransactionSession,
};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[sea_orm(table_name = "segments")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub source: String,
    #[sea_orm(default_value = "xc")]
    pub mode: String,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub distance_meters: Option<f64>,
    #[sea_orm(default_value = false)]
    pub starred: bool,
    pub route_data_json: Option<StoredRoutePointSeries>,
    pub source_activity_id: Option<i32>,
    pub source_start_route_point_index: Option<i32>,
    pub source_end_route_point_index: Option<i32>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub last_activity_change_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::timestamps::impl_timestamps!(ActiveModel);

#[derive(Debug, FromQueryResult)]
pub struct SegmentMatchingRoute {
    pub id: i32,
    pub route_data_json: Option<StoredRoutePointSeries>,
}

#[derive(Debug, FromQueryResult)]
pub struct SegmentTitle {
    pub id: i32,
    pub title: String,
}

impl Model {
    pub async fn find_owned(db: &impl ConnectionTrait, segment_id: i32, user_id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(segment_id)
            .filter(Column::UserId.eq(user_id))
            .one(db)
            .await
    }

    pub async fn find_by_id(db: &impl ConnectionTrait, segment_id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(segment_id).one(db).await
    }

    pub async fn delete_with_derived_state(
        &self,
        db: &(impl ConnectionTrait + sea_orm::TransactionTrait),
    ) -> Result<(), DbErr> {
        let activity_ids = super::segment_efforts::Model::activity_ids_for_segment(db, self.id).await?;
        let txn = db.begin().await?;
        super::activities::Model::lock_for_analytics(&txn, &activity_ids).await?;
        super::segment_user_summaries::Entity::delete_for_segment(&txn, self.id).await?;
        super::segment_summaries::Entity::delete_for_segment(&txn, self.id).await?;
        super::segment_efforts::Entity::delete_for_segment(&txn, self.id).await?;
        Entity::delete_by_id(self.id)
            .filter(Column::UserId.eq(self.user_id))
            .exec(&txn)
            .await?;
        if !activity_ids.is_empty() {
            crate::analytics::rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
        }
        txn.commit().await
    }

    pub async fn metrics_counts(
        db: &DatabaseConnection,
        now: DateTime<Utc>,
    ) -> Result<crate::platform::aggregator::StoredRecordCounts, DbErr> {
        crate::platform::aggregator::Aggregator::stored_records::<Entity>(db, Column::CreatedAt, now).await
    }

    pub async fn mark_activity_changes<C>(db: &C, segment_ids: &[i32], changed_at: DateTime<Utc>) -> Result<(), DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() {
            return Ok(());
        }
        Entity::update_many()
            .col_expr(Column::LastActivityChangeAt, Expr::value(changed_at))
            .col_expr(Column::UpdatedAt, Expr::value(Utc::now()))
            .filter(Column::Id.is_in(segment_ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }

    pub async fn titles_by_ids<C>(db: &C, segment_ids: &[i32]) -> Result<Vec<SegmentTitle>, DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() {
            return Ok(Vec::new());
        }
        Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Title)
            .filter(Column::Id.is_in(segment_ids.iter().copied()))
            .into_model::<SegmentTitle>()
            .all(db)
            .await
    }

    pub async fn matching_routes_page<C>(
        db: &C,
        after_id: i32,
        limit: u64,
        bounds: Option<super::route_matching_bounds::Bounds>,
    ) -> Result<Vec<SegmentMatchingRoute>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .select_only()
            .columns([Column::Id, Column::RouteDataJson])
            .filter(Column::Id.in_subquery(Self::matching_id_page(after_id, limit, bounds)))
            .order_by_asc(Column::Id)
            .into_model::<SegmentMatchingRoute>()
            .all(db)
            .await
    }

    fn matching_id_page(
        after_id: i32,
        limit: u64,
        bounds: Option<super::route_matching_bounds::Bounds>,
    ) -> SelectStatement {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::Id.gt(after_id))
            .apply_if(bounds, |query, bounds| {
                query.filter(Column::Id.in_subquery(Self::candidate_segment_ids(bounds)))
            })
            .order_by_asc(Column::Id)
            .limit(limit.clamp(1, 25))
            .into_query()
    }

    fn candidate_segment_ids(bounds: super::route_matching_bounds::Bounds) -> SelectStatement {
        Self::overlapping_segment_ids(bounds)
            .union(
                UnionType::All,
                Self::segments_without_bounds()
                    .select_only()
                    .column(Column::Id)
                    .into_query(),
            )
            .to_owned()
    }

    fn overlapping_segment_ids(bounds: super::route_matching_bounds::Bounds) -> SelectStatement {
        use super::route_matching_bounds::segment::{Column as BoundsColumn, Entity as BoundsEntity};

        BoundsEntity::find()
            .select_only()
            .column(BoundsColumn::SegmentId)
            // Match the partial GiST index predicate even with a generic prepared plan.
            .filter(BoundsColumn::PointCount.into_expr().gte(Expr::Constant(2.into())))
            .filter(bounds.overlaps(
                [
                    BoundsColumn::MinLongitude.into_expr(),
                    BoundsColumn::MinLatitude.into_expr(),
                ],
                [
                    BoundsColumn::MaxLongitude.into_expr(),
                    BoundsColumn::MaxLatitude.into_expr(),
                ],
            ))
            .into_query()
    }

    pub(super) async fn pending_matching_bounds_ids(
        db: &impl ConnectionTrait,
        after_id: i32,
    ) -> Result<Vec<i32>, DbErr> {
        Self::segments_without_bounds()
            .select_only()
            .column(Column::Id)
            .filter(Column::Id.gt(after_id))
            .order_by_asc(Column::Id)
            .limit(16)
            .into_tuple::<i32>()
            .all(db)
            .await
    }

    fn segments_without_bounds() -> Select<Entity> {
        use super::route_matching_bounds::segment::{Column as BoundsColumn, Entity as BoundsEntity};

        let saved_bounds = BoundsEntity::find()
            .select_only()
            .column(BoundsColumn::SegmentId)
            .filter(Expr::col((BoundsEntity, BoundsColumn::SegmentId)).equals((Entity, Column::Id)))
            .into_query();
        Entity::find().filter(Expr::exists(saved_bounds).not())
    }

    pub async fn list_by_ids<C>(db: &C, segment_ids: &[i32]) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        if segment_ids.is_empty() {
            return Ok(Vec::new());
        }

        Entity::find()
            .filter(Column::Id.is_in(segment_ids.iter().copied()))
            .all(db)
            .await
    }
}

mod access;
pub use access::SegmentListRow;

impl Model {
    pub async fn downhill_for_user(db: &impl ConnectionTrait, user_id: i32) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Mode.eq("dh"))
            .all(db)
            .await
    }
    pub async fn downhill_by_ids(db: &impl ConnectionTrait, segment_ids: &[i32]) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Id.is_in(segment_ids.iter().copied()))
            .filter(Column::Mode.eq("dh"))
            .all(db)
            .await
    }
    pub async fn ids_page(db: &impl ConnectionTrait, after_id: i32, limit: u64) -> Result<Vec<i32>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::Id.gt(after_id))
            .order_by_asc(Column::Id)
            .limit(limit)
            .into_tuple()
            .all(db)
            .await
    }
    pub async fn all_ids(db: &impl ConnectionTrait) -> Result<Vec<i32>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .into_tuple()
            .all(db)
            .await
    }
    pub async fn lock_for_analytics(db: &impl ConnectionTrait, segment_id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(segment_id).lock_exclusive().one(db).await
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(example = "xc")]
pub enum SegmentMode {
    Xc,
    Dh,
}

impl SegmentMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xc => "xc",
            Self::Dh => "dh",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "dh" => Self::Dh,
            _ => Self::Xc,
        }
    }
}

mod mutations;
pub use mutations::{normalize_segment_title, slice_builder_route_points, ActivitySegmentSource, SegmentImport};
