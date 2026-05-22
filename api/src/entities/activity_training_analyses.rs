use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ConnectionTrait, DbErr, Set};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activity_training_analyses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub activity_id: i32,
    pub user_id: i32,
    pub ride_focus: String,
    pub route_family_key: Option<String>,
    pub comparable_distance_bucket_meters: Option<i32>,
    pub comparable_elevation_gain_bucket_meters: Option<i32>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub z2_time_seconds: i32,
    pub z2_distance_meters: Option<f64>,
    pub z2_average_speed_mps: Option<f64>,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: Option<f64>,
    pub sustained_climb_count: i32,
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
