//! Ownership, time, sport, zoom and spatial predicates run before materializing coordinates.
use super::{geometry::zoom_band, raster::Tile, types::HeatmapQuery, PROJECTION_VERSION};
use crate::entities::{
    activities, heatmap_chunks as chunks, heatmap_projections as projections,
    heatmap_user_states as user_states,
};
use sea_orm::sea_query::{Expr, Func, Query};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DbErr, EntityTrait, ExprTrait, FromQueryResult,
    JoinType, QueryFilter, QueryOrder, QuerySelect, RelationTrait, Select,
};

#[derive(Debug, FromQueryResult)]
pub struct Progress {
    pub revision: i64,
    pub ready: i64,
    pub pending: i64,
    pub failed: i64,
    pub skipped: i64,
    pub min_x: Option<f64>,
    pub min_y: Option<f64>,
    pub max_x: Option<f64>,
    pub max_y: Option<f64>,
}

#[derive(FromQueryResult)]
pub struct TileChunk {
    pub activity_id: i32,
    pub chunk_index: i32,
    pub points: Vec<u8>,
}

#[derive(Debug, FromQueryResult)]
pub struct ActivityCenter {
    pub x: f64,
    pub y: f64,
}

pub struct HeatmapData;

impl HeatmapData {
    pub async fn revision(db: &impl ConnectionTrait, user_id: i32) -> Result<i64, DbErr> {
        Ok(user_states::Entity::find_by_id(user_id)
            .select_only()
            .column(user_states::Column::Revision)
            .into_tuple::<i64>()
            .one(db)
            .await?
            .unwrap_or(0))
    }

    pub async fn progress(
        db: &impl ConnectionTrait,
        user_id: i32,
        filters: &HeatmapQuery,
    ) -> Result<Progress, DbErr> {
        let revision = Query::select()
            .column(user_states::Column::Revision)
            .from(user_states::Entity)
            .and_where(user_states::Column::UserId.eq(user_id))
            .to_owned();
        Self::scoped_projections(user_id, filters)
            .expr_as(
                Func::coalesce([revision.into(), Expr::val(0_i64)]),
                "revision",
            )
            .expr_as(Self::status_count("ready"), "ready")
            .expr_as(Self::status_count("pending"), "pending")
            .expr_as(Self::status_count("failed"), "failed")
            .expr_as(Self::status_count("skipped"), "skipped")
            .expr_as(
                Func::min(projections::Column::MinX.into_expr()).filter(Self::ready()),
                "min_x",
            )
            .expr_as(
                Func::min(projections::Column::MinY.into_expr()).filter(Self::ready()),
                "min_y",
            )
            .expr_as(
                Func::max(projections::Column::MaxX.into_expr()).filter(Self::ready()),
                "max_x",
            )
            .expr_as(
                Func::max(projections::Column::MaxY.into_expr()).filter(Self::ready()),
                "max_y",
            )
            .into_model::<Progress>()
            .one(db)
            .await?
            .ok_or_else(|| DbErr::Custom("Missing heatmap progress".into()))
    }

    pub async fn activity_centers(
        db: &impl ConnectionTrait,
        user_id: i32,
        filters: &HeatmapQuery,
    ) -> Result<Vec<ActivityCenter>, DbErr> {
        let x_sum = projections::Column::MinX
            .into_expr()
            .add(projections::Column::MaxX.into_expr());
        let wraps_dateline = projections::Column::MaxX
            .into_expr()
            .sub(projections::Column::MinX.into_expr())
            .gt(0.5);
        let wrapped_x = x_sum
            .clone()
            .add(1.0)
            .div(2.0)
            .sub(Expr::case(x_sum.clone().gte(1.0), 1.0).finally(0.0));
        Self::scoped_projections(user_id, filters)
            .expr_as(
                Expr::case(wraps_dateline, wrapped_x).finally(x_sum.div(2.0)),
                "x",
            )
            .expr_as(
                projections::Column::MinY
                    .into_expr()
                    .add(projections::Column::MaxY.into_expr())
                    .div(2.0),
                "y",
            )
            .filter(Self::ready())
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .order_by_asc(projections::Column::ActivityId)
            .into_model::<ActivityCenter>()
            .all(db)
            .await
    }

    pub async fn tile_page(
        db: &impl ConnectionTrait,
        user_id: i32,
        filters: &HeatmapQuery,
        tile: Tile,
        cursor: (i32, i32),
    ) -> Result<Vec<TileChunk>, DbErr> {
        let boxes = tile
            .query_bounds()
            .into_iter()
            .fold(Condition::any(), |condition, bounds| {
                condition.add(Self::box_overlap(bounds))
            });
        Self::scoped_projections(user_id, filters)
            .columns([
                chunks::Column::ActivityId,
                chunks::Column::ChunkIndex,
                chunks::Column::Points,
            ])
            .join(JoinType::InnerJoin, projections::Relation::Chunks.def())
            .filter(Self::ready())
            .filter(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .filter(chunks::Column::Band.eq(zoom_band(tile.z)))
            .filter(
                Expr::tuple([
                    chunks::Column::ActivityId.into_expr(),
                    chunks::Column::ChunkIndex.into_expr(),
                ])
                .gt(Expr::tuple([Expr::val(cursor.0), Expr::val(cursor.1)])),
            )
            .filter(boxes)
            .order_by_asc(chunks::Column::ActivityId)
            .order_by_asc(chunks::Column::ChunkIndex)
            .limit(64)
            .into_model::<TileChunk>()
            .all(db)
            .await
    }

    fn ready() -> Condition {
        Condition::all()
            .add(projections::Column::Status.eq("ready"))
            .add(projections::Column::ProjectionVersion.eq(PROJECTION_VERSION))
            .add(activities::Model::heatmap_eligible())
    }

    fn status_count(status: &str) -> Expr {
        Func::count(projections::Column::ActivityId.into_expr())
            .filter(if status == "ready" {
                Self::ready()
            } else {
                Condition::all().add(projections::Column::Status.eq(status))
            })
            .into()
    }

    fn scoped_projections(user_id: i32, filters: &HeatmapQuery) -> Select<projections::Entity> {
        let mut query = projections::Entity::find()
            .select_only()
            .join(JoinType::InnerJoin, projections::Relation::Activities.def())
            .filter(activities::Column::UserId.eq(user_id))
            .filter(projections::Column::UserId.eq(user_id));
        if let Some(sport) = filters.sport {
            query = query
                .filter(activities::Column::Sport.is_in(sport.stored_values().iter().copied()));
        }
        if let Some(from) = filters.from {
            query = query.filter(activities::Column::StartedAt.gte(from));
        }
        if let Some(to) = filters.to {
            query = query.filter(activities::Column::StartedAt.lt(to));
        }
        query
    }

    fn box_overlap(bounds: [f64; 4]) -> Expr {
        // PostgreSQL's native box operator matches the expression GiST index;
        // SeaQuery has no typed box/point constructor or overlap operator.
        Expr::cust_with_exprs(
            "box(point($1,$2),point($3,$4)) && box(point($5,$6),point($7,$8))",
            [
                chunks::Column::MinX.into_expr(),
                chunks::Column::MinY.into_expr(),
                chunks::Column::MaxX.into_expr(),
                chunks::Column::MaxY.into_expr(),
                Expr::val(bounds[0]),
                Expr::val(bounds[1]),
                Expr::val(bounds[2]),
                Expr::val(bounds[3]),
            ],
        )
    }
}
