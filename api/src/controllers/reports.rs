use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{activities, activity_training_analyses};
use crate::storage::AppStorage;
use crate::training_profile::deserialize_activity_heart_rate_zones;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
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
    pub report: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
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
    pub ride_summary: Option<RideSummaryReportResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct RideSummaryReportResponse {
    pub activity_count: i32,
    pub total_distance_meters: f64,
    pub total_distance_miles: f64,
    pub total_elevation_gain_meters: f64,
    pub total_elevation_gain_feet: f64,
    pub total_elapsed_seconds: i32,
    pub total_moving_seconds: i32,
    pub total_stopped_seconds: i32,
    pub average_speed_mps: Option<f64>,
    pub average_speed_mph: Option<f64>,
    pub average_heart_rate_bpm: Option<f64>,
    pub max_heart_rate_bpm: Option<i32>,
    pub climbing_density_feet_per_hour: Option<f64>,
    pub z1_seconds: i32,
    pub z2_seconds: i32,
    pub z3_seconds: i32,
    pub z4_seconds: i32,
    pub z5_seconds: i32,
    pub data_quality_flags: Vec<String>,
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
    let (range_start, range_end) = report_range(boundary, now, &query)?;

    let activity_models = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user.id))
        .filter(activities::Column::StartedAt.gte(range_start))
        .filter(activities::Column::StartedAt.lte(range_end))
        .all(&state.db)
        .await?;

    if query.report.as_deref() == Some("ride_summary") {
        return Ok(Json(TrainingReportsResponse {
            generated_at: now,
            boundary,
            range_start: range_start.to_rfc3339(),
            range_end: range_end.to_rfc3339(),
            points: Vec::new(),
            ride_summary: Some(build_ride_summary_report(&activity_models)),
        }));
    }

    let activity_ids = activity_models
        .iter()
        .map(|activity| activity.id)
        .collect::<Vec<_>>();
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

        if let Some(decoupling_percent) =
            analysis.and_then(|model| model.aerobic_decoupling_percent)
        {
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
    while cursor <= range_end {
        let next_cursor = boundary.next_bucket_start(cursor)?;
        let bucket_end = if next_cursor > range_end {
            range_end
        } else {
            next_cursor
        };
        let day_span = ((bucket_end - cursor).num_seconds() as f64 / 86_400.0).max(1.0 / 24.0);
        let accumulator = buckets.get(&cursor).cloned().unwrap_or_default();

        points.push(TrainingReportPointResponse {
            bucket_start: cursor.to_rfc3339(),
            bucket_end: bucket_end.to_rfc3339(),
            z2_average_speed_mps: average_or_none(
                accumulator.z2_speed_sum,
                accumulator.z2_speed_count,
            ),
            average_aerobic_decoupling_percent: average_or_none(
                accumulator.decoupling_sum,
                accumulator.decoupling_count,
            ),
            climbing_pace_feet_per_week: if accumulator.climbing_feet_total > 0.0 {
                Some(round_metric(
                    accumulator.climbing_feet_total * 7.0 / day_span,
                ))
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
        range_end: range_end.to_rfc3339(),
        points,
        ride_summary: None,
    }))
}

fn report_range(
    boundary: ReportBoundary,
    now: DateTime<Utc>,
    query: &TrainingReportsQuery,
) -> Result<(DateTime<Utc>, DateTime<Utc>), AppError> {
    let range_start = match query.start_date {
        Some(date) => make_utc_datetime(date.year(), date.month(), date.day(), 0, 0, 0)?,
        None => boundary.range_start(now),
    };
    let range_end = match query.end_date {
        Some(date) => make_utc_datetime(date.year(), date.month(), date.day(), 23, 59, 59)?,
        None => now,
    };

    if range_start > range_end {
        return Err(AppError::bad_request("start_date must be before end_date"));
    }

    Ok((range_start, range_end))
}

fn build_ride_summary_report(activities: &[activities::Model]) -> RideSummaryReportResponse {
    let mut total_distance_meters = 0.0;
    let mut total_elevation_gain_meters = 0.0;
    let mut total_elapsed_seconds = 0;
    let mut total_moving_seconds = 0;
    let mut total_stopped_seconds = 0;
    let mut weighted_hr_sum = 0.0;
    let mut weighted_hr_seconds = 0;
    let mut max_heart_rate_bpm: Option<i32> = None;
    let mut zone_seconds = [0; 5];
    let mut missing_distance = 0;
    let mut missing_elevation = 0;
    let mut missing_time = 0;
    let mut missing_hr = 0;

    for activity in activities {
        match activity.distance_meters {
            Some(distance) => total_distance_meters += distance.max(0.0),
            None => missing_distance += 1,
        }

        match activity.elevation_gain_meters {
            Some(elevation) => total_elevation_gain_meters += elevation.max(0.0),
            None => missing_elevation += 1,
        }

        let moving_seconds = activity.moving_time_seconds.unwrap_or_default().max(0);
        let elapsed_seconds = activity
            .total_time_seconds
            .or_else(|| {
                activity
                    .ended_at
                    .map(|ended_at| (ended_at - activity.started_at).num_seconds() as i32)
            })
            .unwrap_or(moving_seconds)
            .max(moving_seconds)
            .max(0);

        if elapsed_seconds == 0 && moving_seconds == 0 {
            missing_time += 1;
        }

        total_elapsed_seconds += elapsed_seconds;
        total_moving_seconds += moving_seconds;
        total_stopped_seconds += (elapsed_seconds - moving_seconds).max(0);

        if let Some(avg_hr) = activity.average_heart_rate_bpm {
            let weight = moving_seconds.max(elapsed_seconds).max(1);
            weighted_hr_sum += f64::from(avg_hr) * f64::from(weight);
            weighted_hr_seconds += weight;
        } else {
            missing_hr += 1;
        }

        if let Some(activity_max_hr) = activity.max_heart_rate_bpm {
            max_heart_rate_bpm = Some(
                max_heart_rate_bpm.map_or(activity_max_hr, |current| current.max(activity_max_hr)),
            );
        }

        for zone in deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref()) {
            let zone_index = (zone.zone - 1).clamp(0, 4) as usize;
            zone_seconds[zone_index] += zone.duration_seconds.max(0);
        }
    }

    let moving_hours = f64::from(total_moving_seconds) / 3600.0;
    let average_speed_mps = if total_moving_seconds > 0 && total_distance_meters > 0.0 {
        Some(round_metric(
            total_distance_meters / f64::from(total_moving_seconds),
        ))
    } else {
        None
    };
    let average_heart_rate_bpm = average_or_none(weighted_hr_sum, weighted_hr_seconds);
    let climbing_density_feet_per_hour = if moving_hours > 0.0 && total_elevation_gain_meters > 0.0
    {
        Some(round_metric(
            total_elevation_gain_meters * FEET_PER_METER / moving_hours,
        ))
    } else {
        None
    };

    let mut data_quality_flags = Vec::new();
    push_missing_flag(
        &mut data_quality_flags,
        missing_distance,
        activities.len(),
        "distance",
    );
    push_missing_flag(
        &mut data_quality_flags,
        missing_elevation,
        activities.len(),
        "elevation",
    );
    push_missing_flag(
        &mut data_quality_flags,
        missing_time,
        activities.len(),
        "time",
    );
    push_missing_flag(
        &mut data_quality_flags,
        missing_hr,
        activities.len(),
        "heart rate",
    );

    RideSummaryReportResponse {
        activity_count: activities.len() as i32,
        total_distance_meters: round_metric(total_distance_meters),
        total_distance_miles: round_metric(total_distance_meters / 1609.344),
        total_elevation_gain_meters: round_metric(total_elevation_gain_meters),
        total_elevation_gain_feet: round_metric(total_elevation_gain_meters * FEET_PER_METER),
        total_elapsed_seconds,
        total_moving_seconds,
        total_stopped_seconds,
        average_speed_mps,
        average_speed_mph: average_speed_mps.map(|speed| round_metric(speed * 2.236_936)),
        average_heart_rate_bpm,
        max_heart_rate_bpm,
        climbing_density_feet_per_hour,
        z1_seconds: zone_seconds[0],
        z2_seconds: zone_seconds[1],
        z3_seconds: zone_seconds[2],
        z4_seconds: zone_seconds[3],
        z5_seconds: zone_seconds[4],
        data_quality_flags,
    }
}

fn push_missing_flag(flags: &mut Vec<String>, missing: usize, total: usize, label: &str) {
    if missing == 0 {
        return;
    }

    flags.push(format!(
        "{missing} of {total} activities missing {label} data"
    ));
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
            Self::Day => {
                make_utc_datetime(value.year(), value.month(), value.day(), value.hour(), 0, 0)
            }
            Self::Week | Self::Month => {
                make_utc_datetime(value.year(), value.month(), value.day(), 0, 0, 0)
            }
            Self::ThreeMonth | Self::SixMonth => {
                let date = value.date_naive();
                let monday =
                    date - Duration::days(i64::from(date.weekday().num_days_from_monday()));
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
