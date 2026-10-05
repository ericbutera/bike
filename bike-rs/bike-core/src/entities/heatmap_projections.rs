use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "heatmap_projections")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    pub generation: i64,
    pub status: String,
    pub projection_version: i32,
    pub queued_at: Option<DateTimeUtc>,
    pub error: Option<String>,
    pub min_x: Option<f64>,
    pub min_y: Option<f64>,
    pub max_x: Option<f64>,
    pub max_y: Option<f64>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::activities::Entity",
        from = "Column::ActivityId",
        to = "super::activities::Column::Id"
    )]
    Activities,
    #[sea_orm(has_many = "super::heatmap_chunks::Entity")]
    Chunks,
}

impl ActiveModelBehavior for ActiveModel {}
