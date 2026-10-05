use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_gateway_bindings")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub athlete_id: i64,
    pub user_id: i32,
    pub created_at: DateTimeUtc,
    pub active_delivery_id: Option<String>,
    pub lease_until: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
