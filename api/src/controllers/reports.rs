use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{activities, activity_training_analyses};
use crate::storage::AppStorage;
use crate::training_profile::deserialize_activity_heart_rate_zones;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Datelike, Duration, TimeZone, Timelike, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

const FEET_PER_METER: f64 = 3.28084;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportBoundary {
    Day,
    Week,
    Month,
    #[serde(rename = "3month")]
    ThreeMonth,
    #[serde(rename = "6month")]
    SixMonth,
    #[serde(rename = "1year")]
    OneYear,
    #[serde(rename = "2year")]
    TwoYear,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct TrainingReportsQuery {
    pub boundary: Option<ReportBoundary>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingReportPointResponse {
    pub bucket_start: String,
    pub bucket_end: String,
    pub z2_average_speed_mps: Option<f64>,
    pub average_aerobic_decoupling_percent: Option<f64>,
    pub climbing_pace_feet_per_week: Option<f64>,
    pub z1_seconds: i32,
    pub z2_seconds: i32,
    pub z3_seconds: i32,
    pub z4_seconds: i32,
    pub z5_seconds: i32,
    pub elevation_gain_meters: f64,
    pub elevation_gain_feet: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingReportsResponse {
    pub generated_at: DateTime<Utc>,
    pub boundary: ReportBoundary,
    pub range_start: String,
    pub range_end: String,
    pub points: Vec<TrainingReportPointResponse>,
}

#[derive(Debug, Default, Clone)]
struct BucketAccumulator {
    z2_speed_sum: f64,
    z2_speed_count: i32,
    decoupling_sum: f64,
    decoupling_count: i32,
    climbing_feet_total: f64,
    elevation_meters_total: f64,
    zone_seconds: [i32; 5],
}

#[utoipa::path(
    get,
    path = "/api/training/reports",
    params(TrainingReportsQuery),
    responses(
        (status = 200, description = "Training reports over a selected boundary for the authenticated user", body = TrainingReportsResponse),
        (status = 400, description = "Invalid query parameters", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_training_reports(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<TrainingReportsQuery>,
) -> Result<Json<TrainingReportsResponse>, AppError> {
    let boundary = query.boundary.unwrap_or(ReportBoundary::Month);
    let now = Utc::now();
    let range_start = boundary.range_start(now);

    let activity_models = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user.id))
        .filter(activities::Column::StartedAt.gte(range_start))
        .filter(activities::Column::StartedAt.lte(now))
        .all(&state.db)
        .await?;

    let activity_ids = activity_models.iter().map(|activity| activity.id).collect::<Vec<_>>();
    let analysis_models = if activity_ids.is_empty() {
        Vec::new()
    } else {
        activity_training_analyses::Entity::find()
            .filter(activity_training_analyses::Column::UserId.eq(user.id))
            .filter(activity_training_analyses::Column::ActivityId.is_in(activity_ids))
            .all(&state.db)
            .await?
    };

    let analysis_by_activity = analysis_models
        .into_iter()
        .map(|analysis| (analysis.activity_id, analysis))
        .collect::<HashMap<_, _>>();

    let mut buckets = HashMap::<DateTime<Utc>, BucketAccumulator>::new();
    for activity in &activity_models {
        let bucket_start = boundary.bucket_start(activity.started_at)?;
        let bucket = buckets.entry(bucket_start).or_default();
        let analysis = analysis_by_activity.get(&activity.id);

        if let Some(speed_mps) = analysis.and_then(|model| model.z2_average_speed_mps) {
            bucket.z2_speed_sum += speed_mps;
            bucket.z2_speed_count += 1;
        }

        if let Some(decoupling_percent) = analysis.and_then(|model| model.aerobic_decoupling_percent) {
            bucket.decoupling_sum += decoupling_percent;
            bucket.decoupling_count += 1;
        }

        let climbing_meters = analysis
            .and_then(|model| model.climbing_elevation_gain_meters)
            .or(activity.elevation_gain_meters)
            .unwrap_or(0.0);
        bucket.climbing_feet_total += climbing_meters * FEET_PER_METER;

        let elevation_gain_meters = activity
            .elevation_gain_meters
            .or_else(|| analysis.and_then(|model| model.climbing_elevation_gain_meters))
            .unwrap_or(0.0);
        bucket.elevation_meters_total += elevation_gain_meters;

        let zones = deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref());
        for zone in zones {
            let zone_index = (zone.zone - 1).clamp(0, 4) as usize;
            bucket.zone_seconds[zone_index] += zone.duration_seconds.max(0);
        }
    }

    let mut points = Vec::new();
    let mut cursor = boundary.bucket_start(range_start)?;
    while cursor <= now {
        let next_cursor = boundary.next_bucket_start(cursor)?;
        let bucket_end = if next_cursor > now { now } else { next_cursor };
        let day_span = ((bucket_end - cursor).num_seconds() as f64 / 86_400.0).max(1.0 / 24.0);
        let accumulator = buckets.get(&cursor).cloned().unwrap_or_default();

        points.push(TrainingReportPointResponse {
            bucket_start: cursor.to_rfc3339(),
            bucket_end: bucket_end.to_rfc3339(),
            z2_average_speed_mps: average_or_none(accumulator.z2_speed_sum, accumulator.z2_speed_count),
            average_aerobic_decoupling_percent: average_or_none(
                accumulator.decoupling_sum,
                accumulator.decoupling_count,
            ),
            climbing_pace_feet_per_week: if accumulator.climbing_feet_total > 0.0 {
                Some(round_metric(accumulator.climbing_feet_total * 7.0 / day_span))
            } else {
                None
            },
            z1_seconds: accumulator.zone_seconds[0],
            z2_seconds: accumulator.zone_seconds[1],
            z3_seconds: accumulator.zone_seconds[2],
            z4_seconds: accumulator.zone_seconds[3],
            z5_seconds: accumulator.zone_seconds[4],
            elevation_gain_meters: round_metric(accumulator.elevation_meters_total),
            elevation_gain_feet: round_metric(accumulator.elevation_meters_total * FEET_PER_METER),
        });

        cursor = next_cursor;
    }

    Ok(Json(TrainingReportsResponse {
        generated_at: now,
        boundary,
        range_start: range_start.to_rfc3339(),
        range_end: now.to_rfc3339(),
        points,
    }))
}

impl ReportBoundary {
    fn range_start(self, now: DateTime<Utc>) -> DateTime<Utc> {
        match self {
            Self::Day => now - Duration::days(1),
            Self::Week => now - Duration::days(7),
            Self::Month => now - Duration::days(30),
            Self::ThreeMonth => now - Duration::days(90),
            Self::SixMonth => now - Duration::days(180),
            Self::OneYear => now - Duration::days(365),
            Self::TwoYear => now - Duration::days(730),
        }
    }

    fn bucket_start(self, value: DateTime<Utc>) -> Result<DateTime<Utc>, AppError> {
        match self {
            Self::Day => make_utc_datetime(
                value.year(),
                value.month(),
                value.day(),
                value.hour(),
                0,
                0,
            ),
            Self::Week | Self::Month => {
                make_utc_datetime(value.year(), value.month(), value.day(), 0, 0, 0)
            }
            Self::ThreeMonth | Self::SixMonth => {
                let date = value.date_naive();
                let monday = date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
                make_utc_datetime(monday.year(), monday.month(), monday.day(), 0, 0, 0)
            }
            Self::OneYear | Self::TwoYear => {
                make_utc_datetime(value.year(), value.month(), 1, 0, 0, 0)
            }
        }
    }

    fn next_bucket_start(self, value: DateTime<Utc>) -> Result<DateTime<Utc>, AppError> {
        match self {
            Self::Day => Ok(value + Duration::hours(1)),
            Self::Week | Self::Month => Ok(value + Duration::days(1)),
            Self::ThreeMonth | Self::SixMonth => Ok(value + Duration::days(7)),
            Self::OneYear | Self::TwoYear => {
                let date = value.date_naive();
                let (year, month) = if date.month() == 12 {
                    (date.year() + 1, 1)
                } else {
                    (date.year(), date.month() + 1)
                };
                make_utc_datetime(year, month, 1, 0, 0, 0)
            }
        }
    }
}

fn make_utc_datetime(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> Result<DateTime<Utc>, AppError> {
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .single()
        .ok_or_else(|| AppError::bad_request("Invalid boundary date"))
}

fn average_or_none(sum: f64, count: i32) -> Option<f64> {
    if count > 0 {
        Some(round_metric(sum / f64::from(count)))
    } else {
        None
    }
}

fn round_metric(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}
