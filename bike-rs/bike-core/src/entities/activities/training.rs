use super::*;
#[derive(Clone, Debug, FromQueryResult)]
pub struct ActivitySummaryRow {
    pub id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub activity_type: String,
    pub heart_rate_zones_json: Option<StoredActivityHeartRateZones>,
}
impl Model {
    pub async fn race_summaries(
        db: &impl ConnectionTrait,
        user_id: i32,
    ) -> Result<Vec<ActivitySummaryRow>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Title)
            .column(Column::StartedAt)
            .column(Column::DistanceMeters)
            .column(Column::ElevationGainMeters)
            .column(Column::MovingTimeSeconds)
            .column(Column::TotalTimeSeconds)
            .column(Column::ActivityType)
            .column(Column::HeartRateZonesJson)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .filter(Column::ActivityType.eq(ActivityType::Race.as_str()))
            .into_model::<ActivitySummaryRow>()
            .all(db)
            .await
    }
    pub async fn training_summaries(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_ids: &[i32],
    ) -> Result<HashMap<i32, ActivitySummaryRow>, DbErr> {
        if activity_ids.is_empty() {
            return Ok(HashMap::new());
        }

        Ok(Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Title)
            .column(Column::StartedAt)
            .column(Column::DistanceMeters)
            .column(Column::ElevationGainMeters)
            .column(Column::MovingTimeSeconds)
            .column(Column::TotalTimeSeconds)
            .column(Column::ActivityType)
            .column(Column::HeartRateZonesJson)
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()))
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .into_model::<ActivitySummaryRow>()
            .all(db)
            .await?
            .into_iter()
            .map(|activity| (activity.id, activity))
            .collect())
    }
}
