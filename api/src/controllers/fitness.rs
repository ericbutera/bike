use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::activities;
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_activity_heart_rate_zones, weighted_zone_intensity,
};
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

const FITNESS_WINDOW_DAYS: f64 = 42.0;
const FATIGUE_WINDOW_DAYS: f64 = 7.0;
const DEFAULT_HEART_RATE_RATIO: f64 = 0.6;

#[derive(Debug, Deserialize, IntoParams)]
pub struct FitnessQuery {
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FitnessFreshnessPoint {
    pub date: String,
    pub training_load: f64,
    pub fitness: f64,
    pub fatigue: f64,
    pub form: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FitnessFreshnessResponse {
    pub start_date: String,
    pub end_date: String,
    pub fitness_window_days: i32,
    pub fatigue_window_days: i32,
    pub points: Vec<FitnessFreshnessPoint>,
}

#[utoipa::path(
    get,
    path = "/api/fitness",
    params(FitnessQuery),
    responses(
        (status = 200, description = "Daily fitness, fatigue, and form data for the authenticated user", body = FitnessFreshnessResponse),
        (status = 400, description = "Invalid query parameters", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "fitness",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_fitness_freshness(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<FitnessQuery>,
) -> Result<Json<FitnessFreshnessResponse>, AppError> {
    let end_date = query
        .end_date
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());
    let end_bound = end_of_day_utc(end_date)?;

    let activity_models = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user.id))
        .filter(activities::Column::StartedAt.lte(end_bound))
        .order_by_asc(activities::Column::StartedAt)
        .all(&state.db)
        .await?;

    let earliest_activity_date = activity_models
        .first()
        .map(|activity| activity.started_at.date_naive());
    let requested_start_date = query
        .start_date
        .as_deref()
        .map(parse_date)
        .transpose()?
        .unwrap_or_else(|| earliest_activity_date.unwrap_or(end_date));

    if requested_start_date > end_date {
        return Err(AppError::validation_field(
            "start_date",
            "Start date must be on or before end date",
        ));
    }

    let points = build_fitness_freshness_points(
        &activity_models,
        requested_start_date,
        end_date,
    );

    Ok(Json(FitnessFreshnessResponse {
        start_date: requested_start_date.format("%Y-%m-%d").to_string(),
        end_date: end_date.format("%Y-%m-%d").to_string(),
        fitness_window_days: FITNESS_WINDOW_DAYS as i32,
        fatigue_window_days: FATIGUE_WINDOW_DAYS as i32,
        points,
    }))
}

fn build_fitness_freshness_points(
    activities: &[activities::Model],
    requested_start_date: NaiveDate,
    end_date: NaiveDate,
) -> Vec<FitnessFreshnessPoint> {
    let mut training_load_by_date = BTreeMap::<NaiveDate, f64>::new();

    for activity in activities {
        if let Some(training_load) = estimated_training_load(activity) {
            *training_load_by_date
                .entry(activity.started_at.date_naive())
                .or_insert(0.0) += training_load;
        }
    }

    let compute_start_date = activities
        .first()
        .map(|activity| activity.started_at.date_naive().min(requested_start_date))
        .unwrap_or(requested_start_date);

    let mut current_date = compute_start_date;
    let mut fitness = 0.0;
    let mut fatigue = 0.0;
    let mut points = Vec::new();

    while current_date <= end_date {
        let training_load = *training_load_by_date.get(&current_date).unwrap_or(&0.0);
        fitness += (training_load - fitness) / FITNESS_WINDOW_DAYS;
        fatigue += (training_load - fatigue) / FATIGUE_WINDOW_DAYS;
        let form = fitness - fatigue;

        if current_date >= requested_start_date {
            points.push(FitnessFreshnessPoint {
                date: current_date.format("%Y-%m-%d").to_string(),
                training_load: round_metric(training_load),
                fitness: round_metric(fitness),
                fatigue: round_metric(fatigue),
                form: round_metric(form),
            });
        }

        current_date += Duration::days(1);
    }

    points
}

fn estimated_training_load(activity: &activities::Model) -> Option<f64> {
    let duration_seconds = activity
        .moving_time_seconds
        .or(activity.total_time_seconds)
        .filter(|value| *value > 0)?;
    let duration_hours = f64::from(duration_seconds) / 3600.0;
    let heart_rate_ratio = weighted_zone_intensity(
        &deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_deref()),
    )
    .unwrap_or_else(|| estimated_heart_rate_ratio(activity));

    Some(duration_hours * 100.0 * heart_rate_ratio.powi(2))
}

fn estimated_heart_rate_ratio(activity: &activities::Model) -> f64 {
    match (activity.average_heart_rate_bpm, activity.max_heart_rate_bpm) {
        (Some(average), Some(maximum)) if maximum > 0 => {
            (f64::from(average) / f64::from(maximum)).clamp(0.35, 1.0)
        }
        (Some(average), _) => (f64::from(average) / 190.0).clamp(0.35, 1.0),
        _ => DEFAULT_HEART_RATE_RATIO,
    }
}

fn parse_date(value: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        AppError::validation_field("date", "Dates must use YYYY-MM-DD format")
    })
}

fn end_of_day_utc(date: NaiveDate) -> Result<DateTime<Utc>, AppError> {
    let naive = date
        .and_hms_opt(23, 59, 59)
        .ok_or_else(|| AppError::validation_field("end_date", "Invalid end date"))?;

    Ok(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc))
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_activity(
        started_at: &str,
        moving_time_seconds: Option<i32>,
        average_heart_rate_bpm: Option<i32>,
        max_heart_rate_bpm: Option<i32>,
    ) -> activities::Model {
        let timestamp = DateTime::parse_from_rfc3339(started_at)
            .unwrap()
            .with_timezone(&Utc);

        activities::Model {
            id: 1,
            user_id: 1,
            activity_import_id: None,
            title: "Lunch Ride".to_string(),
            sport: "Ride".to_string(),
            source: "manual_upload".to_string(),
            original_filename: None,
            format: Some("fit".to_string()),
            started_at: timestamp,
            ended_at: None,
            distance_meters: Some(40000.0),
            moving_time_seconds,
            total_time_seconds: moving_time_seconds,
            elevation_gain_meters: Some(500.0),
            elevation_loss_meters: Some(500.0),
            average_speed_mps: Some(8.0),
            max_speed_mps: Some(12.0),
            average_heart_rate_bpm,
            max_heart_rate_bpm,
            average_cadence_rpm: Some(85),
            max_cadence_rpm: Some(105),
            calories: Some(850),
            estimated_ftp_watts: None,
            heart_rate_zones_json: None,
            derived_data_json: None,
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    #[test]
    fn estimated_training_load_scales_with_duration_and_heart_rate() {
        let easy = make_activity(
            "2026-05-01T12:00:00Z",
            Some(3600),
            Some(120),
            Some(170),
        );
        let hard = make_activity(
            "2026-05-01T12:00:00Z",
            Some(5400),
            Some(155),
            Some(170),
        );

        let easy_load = estimated_training_load(&easy).unwrap();
        let hard_load = estimated_training_load(&hard).unwrap();

        assert!(hard_load > easy_load);
        assert_eq!(round_metric(easy_load), 49.8);
        assert_eq!(round_metric(hard_load), 124.7);
    }

    #[test]
    fn builds_daily_series_with_decay_and_history() {
        let activities = vec![
            make_activity(
                "2026-05-01T12:00:00Z",
                Some(3600),
                Some(140),
                Some(170),
            ),
            make_activity(
                "2026-05-03T12:00:00Z",
                Some(7200),
                Some(145),
                Some(170),
            ),
        ];

        let points = build_fitness_freshness_points(
            &activities,
            NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
            NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
        );

        assert_eq!(points.len(), 3);
        assert_eq!(points[0].date, "2026-05-02");
        assert_eq!(points[0].training_load, 0.0);
        assert!(points[0].fitness > 0.0);
        assert!(points[0].fatigue > points[0].fitness);
        assert!(points[1].training_load > 0.0);
        assert!(points[1].fatigue > points[0].fatigue);
        assert!(points[2].form < 0.0);
    }

    #[test]
    fn parse_date_requires_iso_format() {
        let error = parse_date("05/01/2026").unwrap_err();

        assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Dates must use YYYY-MM-DD format");
    }
}