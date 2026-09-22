use crate::activity_details::StoredActivityDerivedData;
use crate::training_profile::StoredActivityHeartRateZones;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Set,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "activities")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub user_id: i32,
    pub activity_import_id: Option<i32>,
    pub title: String,
    pub sport: String,
    pub source: String,
    pub source_correlation_id: Option<String>,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub activity_type: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub distance_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zones_json: Option<StoredActivityHeartRateZones>,
    pub derived_data_json: Option<StoredActivityDerivedData>,
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

#[derive(Clone, Debug, FromQueryResult)]
pub struct ActivityStartedAtRow {
    pub id: i32,
    pub started_at: DateTime<Utc>,
}

#[derive(Clone, Debug, FromQueryResult)]
pub struct ActivityTrainingLoadRow {
    pub started_at: DateTime<Utc>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub heart_rate_zones_json: Option<StoredActivityHeartRateZones>,
}

impl Model {
    pub async fn list_training_loads_for_fitness_rebuild<C>(
        db: &C,
        user_id: i32,
        rebuild_from_day: Option<NaiveDate>,
    ) -> Result<Vec<ActivityTrainingLoadRow>, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut query = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .order_by_asc(Column::StartedAt);

        if let Some(rebuild_from_day) = rebuild_from_day {
            let start_bound = DateTime::<Utc>::from_naive_utc_and_offset(
                rebuild_from_day
                    .and_hms_opt(0, 0, 0)
                    .expect("valid start of day"),
                Utc,
            );
            query = query.filter(Column::StartedAt.gte(start_bound));
        }

        query
            .select_only()
            .column(Column::StartedAt)
            .column(Column::MovingTimeSeconds)
            .column(Column::TotalTimeSeconds)
            .column(Column::AverageHeartRateBpm)
            .column(Column::MaxHeartRateBpm)
            .column(Column::HeartRateZonesJson)
            .into_model::<ActivityTrainingLoadRow>()
            .all(db)
            .await
    }

    pub async fn started_at_by_ids<C>(
        db: &C,
        activity_ids: &[i32],
    ) -> Result<HashMap<i32, DateTime<Utc>>, DbErr>
    where
        C: ConnectionTrait,
    {
        if activity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::StartedAt)
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .into_model::<ActivityStartedAtRow>()
            .all(db)
            .await?
            .into_iter()
            .map(|activity| (activity.id, activity.started_at))
            .collect())
    }
}
