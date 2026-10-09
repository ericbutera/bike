use crate::activity_data::StoredRoutePointSeries;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set,
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
    pub mode: String,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub distance_meters: Option<f64>,
    pub starred: bool,
    pub route_data_json: Option<StoredRoutePointSeries>,
    pub source_activity_id: Option<i32>,
    pub source_start_route_point_index: Option<i32>,
    pub source_end_route_point_index: Option<i32>,
    pub last_activity_change_at: DateTime<Utc>,
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
    pub async fn mark_activity_changes<C>(
        db: &C,
        segment_ids: &[i32],
        changed_at: DateTime<Utc>,
    ) -> Result<(), DbErr>
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
    ) -> Result<Vec<SegmentMatchingRoute>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::RouteDataJson)
            .filter(Column::Id.gt(after_id))
            .order_by_asc(Column::Id)
            .limit(limit)
            .into_model::<SegmentMatchingRoute>()
            .all(db)
            .await
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
