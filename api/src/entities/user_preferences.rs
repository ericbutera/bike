use crate::training_profile::StoredHeartRateZoneBounds;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "user_preferences")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub unit_system: String,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_json: Option<StoredHeartRateZoneBounds>,
    pub xc_goal_start_date: Option<NaiveDate>,
    pub xc_goal_target_date: Option<NaiveDate>,
    pub xc_goal_target_distance_meters: Option<f64>,
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
    pub xc_goal_backfill_status: Option<String>,
    pub xc_goal_backfill_completed_at: Option<DateTime<Utc>>,
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
