use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_gateway_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub delivery_id: String,
    pub athlete_id: i64,
    pub activity_id: i64,
    pub event_time: i64,
    pub operation: String,
    pub status: String,
    pub lease_until: Option<DateTimeUtc>,
    pub created_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
