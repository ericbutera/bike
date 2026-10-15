use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "heatmap_chunks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub band: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub chunk_index: i32,
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
    pub points: Vec<u8>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::heatmap_projections::Entity",
        from = "Column::ActivityId",
        to = "super::heatmap_projections::Column::ActivityId"
    )]
    Projection,
}

impl Related<super::heatmap_projections::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Projection.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

use crate::heatmaps::geometry::Chunk;
use sea_orm::{ConnectionTrait, DbErr, QueryFilter, Set};
impl Model {
    pub async fn insert_chunks(
        db: &impl ConnectionTrait,
        activity_id: i32,
        offset: usize,
        chunks: &[Chunk],
    ) -> Result<(), DbErr> {
        Entity::insert_many(chunks.iter().enumerate().map(|(i, chunk)| ActiveModel {
            activity_id: Set(activity_id),
            band: Set(chunk.band),
            chunk_index: Set((offset + i) as i32),
            min_x: Set(chunk.bounds[0]),
            min_y: Set(chunk.bounds[1]),
            max_x: Set(chunk.bounds[2]),
            max_y: Set(chunk.bounds[3]),
            points: Set(chunk.points.clone()),
        }))
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
    pub async fn clear_activity(db: &impl ConnectionTrait, activity_id: i32) -> Result<(), DbErr> {
        Entity::delete_many()
            .filter(Column::ActivityId.eq(activity_id))
            .exec(db)
            .await?;
        Ok(())
    }
}
