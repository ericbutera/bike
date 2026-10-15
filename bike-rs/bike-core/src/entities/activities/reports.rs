use super::*;
use sea_orm::{Condition, Select};
impl Model {
    fn report_query(
        user_id: i32,
        min_duration_seconds: Option<i32>,
        min_distance_meters: Option<f64>,
    ) -> Select<Entity> {
        let mut query = Entity::find()
            .filter(Column::UserId.eq(user_id))
            .filter(Column::Sport.is_in(BIKE_ACTIVITY_SPORT_VALUES.iter().copied()));
        if let Some(minimum) = min_duration_seconds.filter(|value| *value > 0) {
            query = query.filter(
                Condition::any()
                    .add(Column::MovingTimeSeconds.gte(minimum))
                    .add(Column::TotalTimeSeconds.gte(minimum)),
            );
        }
        if let Some(minimum) = min_distance_meters.filter(|value| *value > 0.0) {
            query = query.filter(Column::DistanceMeters.gte(minimum));
        }
        query
    }
    pub async fn report_range(
        db: &impl ConnectionTrait,
        user_id: i32,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        min_duration_seconds: Option<i32>,
        min_distance_meters: Option<f64>,
    ) -> Result<Vec<Self>, DbErr> {
        Self::report_query(user_id, min_duration_seconds, min_distance_meters)
            .filter(Column::StartedAt.gte(start))
            .filter(Column::StartedAt.lte(end))
            .all(db)
            .await
    }
    pub async fn report_selection(
        db: &impl ConnectionTrait,
        user_id: i32,
        activity_ids: &[i32],
        min_duration_seconds: Option<i32>,
        min_distance_meters: Option<f64>,
    ) -> Result<Vec<Self>, DbErr> {
        Self::report_query(user_id, min_duration_seconds, min_distance_meters)
            .filter(Column::Id.is_in(activity_ids.iter().copied()))
            .all(db)
            .await
    }
}
