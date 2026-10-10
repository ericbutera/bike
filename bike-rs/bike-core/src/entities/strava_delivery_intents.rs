//! Temporary source ownership for accepted deliveries, never a diagnostic payload copy.
use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "strava_delivery_intents")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub delivery_id: String,
    pub content_hash: String,
    pub source: Option<Json>,
    pub accepted_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
