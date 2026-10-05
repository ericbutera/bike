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
