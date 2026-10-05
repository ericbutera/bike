use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_gateway_watermarks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub athlete_id: i64,
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i64,
    pub event_time: i64,
    pub operation: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
