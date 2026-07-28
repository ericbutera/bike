use crate::activity_details::{
    deserialize_derived_activity_data, ActivityChartPoint, ActivityRoutePoint,
};
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
    pub activity_ids: Option<String>,
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
    pub endurance: Option<EnduranceReportResponse>,
    pub climbing: Option<ClimbingReportResponse>,
    pub fatigue: Option<FatigueReportResponse>,
    pub compare_rides: Option<CompareRidesReportResponse>,
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

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EnduranceReportResponse {
    pub activity_count: i32,
    pub median_aerobic_decoupling_percent: Option<f64>,
    pub median_late_speed_change_percent: Option<f64>,
    pub rides: Vec<EnduranceRideResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct EnduranceRideResponse {
    pub activity_id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub elapsed_seconds: i32,
    pub first_half_efficiency_mps_per_bpm: Option<f64>,
    pub second_half_efficiency_mps_per_bpm: Option<f64>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub late_speed_change_percent: Option<f64>,
    pub late_heart_rate_change_percent: Option<f64>,
    pub hourly: Vec<HourlyDurabilityResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FatigueReportResponse {
    pub activity_count: i32,
    pub rides: Vec<FatigueRideResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct FatigueRideResponse {
    pub activity_id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub elapsed_seconds: i32,
    pub fatigue_start_hour: Option<i32>,
    pub hourly: Vec<HourlyDurabilityResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HourlyDurabilityResponse {
    pub hour: i32,
    pub elapsed_start_seconds: i32,
    pub elapsed_end_seconds: i32,
    pub distance_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<f64>,
    pub max_heart_rate_bpm: Option<i32>,
    pub ascent_meters: f64,
    pub moving_seconds: i32,
    pub stopped_seconds: i32,
    pub stop_count: i32,
    pub efficiency_mps_per_bpm: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ClimbingReportResponse {
    pub activity_count: i32,
    pub climb_count: i32,
    pub longest_climb: Option<ClimbResponse>,
    pub fastest_vertical_rate: Option<ClimbResponse>,
    pub median_climb: Option<ClimbResponse>,
    pub percentile_95_climb: Option<ClimbResponse>,
    pub first_half_median: Option<ClimbResponse>,
    pub second_half_median: Option<ClimbResponse>,
    pub best_climb: Option<ClimbResponse>,
    pub worst_climb: Option<ClimbResponse>,
    pub climbs: Vec<ClimbResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ClimbResponse {
    pub activity_id: i32,
    pub activity_title: String,
    pub climb_number: i32,
    pub start_seconds: i32,
    pub summit_seconds: i32,
    pub duration_seconds: i32,
    pub distance_meters: f64,
    pub gain_meters: f64,
    pub average_grade_percent: Option<f64>,
    pub vertical_rate_meters_per_hour: f64,
    pub average_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<f64>,
    pub peak_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_power_watts: Option<f64>,
    pub heart_rate_recovery_30_seconds_bpm: Option<i32>,
    pub heart_rate_recovery_60_seconds_bpm: Option<i32>,
    pub seconds_to_drop_10_bpm: Option<i32>,
    pub seconds_to_drop_15_bpm: Option<i32>,
    pub summit_immediately_enters_descent: bool,
    pub first_or_second_half: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRidesReportResponse {
    pub candidates: Vec<CompareRideCandidateResponse>,
    pub selected_rides: Vec<CompareRideColumnResponse>,
    pub metrics: Vec<CompareRideMetricResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRideCandidateResponse {
    pub activity_id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRideColumnResponse {
    pub activity_id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub elapsed_seconds: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRideMetricResponse {
    pub key: String,
    pub label: String,
    pub unit: Option<String>,
    pub direction: String,
    pub values: Vec<CompareRideMetricValueResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRideMetricValueResponse {
    pub activity_id: i32,
    pub value: Option<f64>,
    pub display: String,
}

#[derive(Debug, Clone, Copy)]
struct TrendSample {
    elapsed_seconds: i32,
    distance_meters: Option<f64>,
    elevation_meters: Option<f64>,
    speed_mps: Option<f64>,
    heart_rate_bpm: Option<i32>,
    cadence_rpm: Option<i32>,
    power_watts: Option<i32>,
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

    match query.report.as_deref() {
        Some("ride_summary") => {
            return Ok(Json(TrainingReportsResponse {
                generated_at: now,
                boundary,
                range_start: range_start.to_rfc3339(),
                range_end: range_end.to_rfc3339(),
                points: Vec::new(),
                ride_summary: Some(build_ride_summary_report(&activity_models)),
                endurance: None,
                climbing: None,
                fatigue: None,
                compare_rides: None,
            }));
        }
        Some("endurance") => {
            return Ok(Json(TrainingReportsResponse {
                generated_at: now,
                boundary,
                range_start: range_start.to_rfc3339(),
                range_end: range_end.to_rfc3339(),
                points: Vec::new(),
                ride_summary: None,
                endurance: Some(build_endurance_report(&activity_models)),
                climbing: None,
                fatigue: None,
                compare_rides: None,
            }));
        }
        Some("climbing") => {
            return Ok(Json(TrainingReportsResponse {
                generated_at: now,
                boundary,
                range_start: range_start.to_rfc3339(),
                range_end: range_end.to_rfc3339(),
                points: Vec::new(),
                ride_summary: None,
                endurance: None,
                climbing: Some(build_climbing_report(&activity_models)),
                fatigue: None,
                compare_rides: None,
            }));
        }
        Some("fatigue") => {
            return Ok(Json(TrainingReportsResponse {
                generated_at: now,
                boundary,
                range_start: range_start.to_rfc3339(),
                range_end: range_end.to_rfc3339(),
                points: Vec::new(),
                ride_summary: None,
                endurance: None,
                climbing: None,
                fatigue: Some(build_fatigue_report(&activity_models)),
                compare_rides: None,
            }));
        }
        Some("compare_rides") => {
            let selected_ids = parse_activity_ids(query.activity_ids.as_deref())?;
            let selected_models = if selected_ids.is_empty() {
                Vec::new()
            } else {
                activities::Entity::find()
                    .filter(activities::Column::UserId.eq(user.id))
                    .filter(activities::Column::Id.is_in(selected_ids))
                    .all(&state.db)
                    .await?
            };

            return Ok(Json(TrainingReportsResponse {
                generated_at: now,
                boundary,
                range_start: range_start.to_rfc3339(),
                range_end: range_end.to_rfc3339(),
                points: Vec::new(),
                ride_summary: None,
                endurance: None,
                climbing: None,
                fatigue: None,
                compare_rides: Some(build_compare_rides_report(
                    &activity_models,
                    &selected_models,
                )),
            }));
        }
        _ => {}
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
        endurance: None,
        climbing: None,
        fatigue: None,
        compare_rides: None,
    }))
}

fn parse_activity_ids(raw: Option<&str>) -> Result<Vec<i32>, AppError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let mut ids = Vec::new();
    for part in raw
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        let id = part
            .parse::<i32>()
            .map_err(|_| AppError::bad_request("activity_ids must be comma-separated integers"))?;
        if id <= 0 {
            return Err(AppError::bad_request(
                "activity_ids must be positive integers",
            ));
        }
        ids.push(id);
    }
    ids.sort_unstable();
    ids.dedup();
    Ok(ids)
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

fn build_endurance_report(activities: &[activities::Model]) -> EnduranceReportResponse {
    let rides = activities
        .iter()
        .filter_map(|activity| {
            let samples = activity_samples(activity);
            if samples.len() < 2 {
                return None;
            }

            let elapsed_seconds = samples.last()?.elapsed_seconds;
            let first = segment_samples(&samples, 0, elapsed_seconds / 2);
            let second = segment_samples(&samples, elapsed_seconds / 2, elapsed_seconds);
            let first_efficiency = efficiency(&first);
            let second_efficiency = efficiency(&second);
            let late = late_ride_changes(&samples);

            Some(EnduranceRideResponse {
                activity_id: activity.id,
                title: activity.title.clone(),
                started_at: activity.started_at,
                elapsed_seconds,
                first_half_efficiency_mps_per_bpm: first_efficiency.map(round_metric),
                second_half_efficiency_mps_per_bpm: second_efficiency.map(round_metric),
                aerobic_decoupling_percent: percent_change(first_efficiency, second_efficiency)
                    .map(round_metric),
                late_speed_change_percent: late.0.map(round_metric),
                late_heart_rate_change_percent: late.1.map(round_metric),
                hourly: hourly_durability(&samples),
            })
        })
        .collect::<Vec<_>>();

    EnduranceReportResponse {
        activity_count: rides.len() as i32,
        median_aerobic_decoupling_percent: median(
            rides
                .iter()
                .filter_map(|ride| ride.aerobic_decoupling_percent)
                .collect(),
        )
        .map(round_metric),
        median_late_speed_change_percent: median(
            rides
                .iter()
                .filter_map(|ride| ride.late_speed_change_percent)
                .collect(),
        )
        .map(round_metric),
        rides,
    }
}

fn build_fatigue_report(activities: &[activities::Model]) -> FatigueReportResponse {
    let rides = activities
        .iter()
        .filter_map(|activity| {
            let samples = activity_samples(activity);
            if samples.len() < 2 {
                return None;
            }

            let hourly = hourly_durability(&samples);
            let fatigue_start_hour = fatigue_start_hour(&hourly);

            Some(FatigueRideResponse {
                activity_id: activity.id,
                title: activity.title.clone(),
                started_at: activity.started_at,
                elapsed_seconds: samples.last()?.elapsed_seconds,
                fatigue_start_hour,
                hourly,
            })
        })
        .collect::<Vec<_>>();

    FatigueReportResponse {
        activity_count: rides.len() as i32,
        rides,
    }
}

fn build_climbing_report(activities: &[activities::Model]) -> ClimbingReportResponse {
    let mut climbs = Vec::new();

    for activity in activities {
        let samples = activity_samples(activity);
        climbs.extend(detect_climbs(activity, &samples));
    }

    climbs.sort_by_key(|climb| (climb.activity_id, climb.climb_number));

    let first_half_climbs = climbs
        .iter()
        .filter(|climb| climb.first_or_second_half == "first")
        .cloned()
        .collect::<Vec<_>>();
    let second_half_climbs = climbs
        .iter()
        .filter(|climb| climb.first_or_second_half == "second")
        .cloned()
        .collect::<Vec<_>>();

    ClimbingReportResponse {
        activity_count: activities.len() as i32,
        climb_count: climbs.len() as i32,
        longest_climb: max_by_metric(&climbs, |climb| f64::from(climb.duration_seconds)),
        fastest_vertical_rate: max_by_metric(&climbs, |climb| climb.vertical_rate_meters_per_hour),
        median_climb: percentile_by_metric(&climbs, 0.50, |climb| climb.gain_meters),
        percentile_95_climb: percentile_by_metric(&climbs, 0.95, |climb| climb.gain_meters),
        first_half_median: percentile_by_metric(&first_half_climbs, 0.50, |climb| {
            climb.vertical_rate_meters_per_hour
        }),
        second_half_median: percentile_by_metric(&second_half_climbs, 0.50, |climb| {
            climb.vertical_rate_meters_per_hour
        }),
        best_climb: max_by_metric(&climbs, |climb| climb.vertical_rate_meters_per_hour),
        worst_climb: min_by_metric(&climbs, |climb| climb.vertical_rate_meters_per_hour),
        climbs,
    }
}

fn build_compare_rides_report(
    candidates: &[activities::Model],
    selected: &[activities::Model],
) -> CompareRidesReportResponse {
    let mut candidate_rows = candidates
        .iter()
        .map(compare_candidate_from_activity)
        .collect::<Vec<_>>();
    candidate_rows.sort_by(|a, b| b.started_at.cmp(&a.started_at));

    let selected_rides = selected
        .iter()
        .map(compare_column_from_activity)
        .collect::<Vec<_>>();

    let analyses = selected
        .iter()
        .map(compare_analysis_from_activity)
        .collect::<Vec<_>>();

    let metrics = vec![
        compare_metric(
            "distance_miles",
            "Distance",
            Some("mi"),
            "neutral",
            &analyses,
            |analysis| analysis.distance_meters.map(|value| value / 1609.344),
        ),
        compare_metric(
            "elevation_gain_feet",
            "Elevation",
            Some("ft"),
            "neutral",
            &analyses,
            |analysis| {
                analysis
                    .elevation_gain_meters
                    .map(|value| value * FEET_PER_METER)
            },
        ),
        compare_metric(
            "elapsed_hours",
            "Elapsed Time",
            Some("h"),
            "neutral",
            &analyses,
            |analysis| Some(f64::from(analysis.elapsed_seconds) / 3600.0),
        ),
        compare_metric(
            "moving_hours",
            "Moving Time",
            Some("h"),
            "neutral",
            &analyses,
            |analysis| {
                analysis
                    .moving_seconds
                    .map(|value| f64::from(value) / 3600.0)
            },
        ),
        compare_metric(
            "aerobic_decoupling_percent",
            "Aerobic Decoupling",
            Some("%"),
            "lower",
            &analyses,
            |analysis| analysis.aerobic_decoupling_percent,
        ),
        compare_metric(
            "median_climb_rate_mph",
            "Median Climb Rate",
            Some("m/h"),
            "higher",
            &analyses,
            |analysis| analysis.median_climb_rate,
        ),
        compare_metric(
            "late_speed_change_percent",
            "Late Ride Fade",
            Some("%"),
            "higher",
            &analyses,
            |analysis| analysis.late_speed_change_percent,
        ),
        compare_metric(
            "stopped_time_percent",
            "Stopped Time",
            Some("%"),
            "lower",
            &analyses,
            |analysis| analysis.stopped_time_percent,
        ),
        compare_metric(
            "climbing_density_feet_per_hour",
            "Climbing Density",
            Some("ft/h"),
            "neutral",
            &analyses,
            |analysis| analysis.climbing_density_feet_per_hour,
        ),
        compare_metric(
            "z2_time_hours",
            "Z2 Time",
            Some("h"),
            "neutral",
            &analyses,
            |analysis| Some(f64::from(analysis.z2_seconds) / 3600.0),
        ),
    ];

    CompareRidesReportResponse {
        candidates: candidate_rows,
        selected_rides,
        metrics,
    }
}

#[derive(Debug, Clone)]
struct CompareRideAnalysis {
    activity_id: i32,
    distance_meters: Option<f64>,
    elevation_gain_meters: Option<f64>,
    elapsed_seconds: i32,
    moving_seconds: Option<i32>,
    aerobic_decoupling_percent: Option<f64>,
    median_climb_rate: Option<f64>,
    late_speed_change_percent: Option<f64>,
    stopped_time_percent: Option<f64>,
    climbing_density_feet_per_hour: Option<f64>,
    z2_seconds: i32,
}

fn compare_analysis_from_activity(activity: &activities::Model) -> CompareRideAnalysis {
    let samples = activity_samples(activity);
    let elapsed_seconds = activity_elapsed_seconds(activity, &samples);
    let moving_seconds = activity.moving_time_seconds;
    let first = segment_samples(&samples, 0, elapsed_seconds / 2);
    let second = segment_samples(&samples, elapsed_seconds / 2, elapsed_seconds);
    let aerobic_decoupling_percent = percent_change(efficiency(&first), efficiency(&second));
    let late_speed_change_percent = late_ride_changes(&samples).0;
    let climbs = detect_climbs(activity, &samples);
    let median_climb_rate = median(
        climbs
            .iter()
            .map(|climb| climb.vertical_rate_meters_per_hour)
            .collect(),
    );
    let stopped_seconds = activity
        .total_time_seconds
        .zip(activity.moving_time_seconds)
        .map(|(total, moving)| (total - moving).max(0));
    let stopped_time_percent =
        stopped_seconds
            .zip(activity.total_time_seconds)
            .and_then(|(stopped, total)| {
                (total > 0).then_some((f64::from(stopped) / f64::from(total)) * 100.0)
            });
    let moving_hours = moving_seconds.map(|value| f64::from(value) / 3600.0);
    let climbing_density_feet_per_hour = activity
        .elevation_gain_meters
        .zip(moving_hours)
        .and_then(|(gain, hours)| (hours > 0.0).then_some((gain * FEET_PER_METER) / hours));
    let z2_seconds = deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref())
        .into_iter()
        .find(|zone| zone.zone == 2)
        .map(|zone| zone.duration_seconds.max(0))
        .unwrap_or_default();

    CompareRideAnalysis {
        activity_id: activity.id,
        distance_meters: activity.distance_meters,
        elevation_gain_meters: activity.elevation_gain_meters,
        elapsed_seconds,
        moving_seconds,
        aerobic_decoupling_percent,
        median_climb_rate,
        late_speed_change_percent,
        stopped_time_percent,
        climbing_density_feet_per_hour,
        z2_seconds,
    }
}

fn compare_candidate_from_activity(activity: &activities::Model) -> CompareRideCandidateResponse {
    CompareRideCandidateResponse {
        activity_id: activity.id,
        title: activity.title.clone(),
        started_at: activity.started_at,
        distance_meters: activity.distance_meters.map(round_metric),
        elevation_gain_meters: activity.elevation_gain_meters.map(round_metric),
        moving_time_seconds: activity.moving_time_seconds,
        total_time_seconds: activity.total_time_seconds,
    }
}

fn compare_column_from_activity(activity: &activities::Model) -> CompareRideColumnResponse {
    let samples = activity_samples(activity);
    CompareRideColumnResponse {
        activity_id: activity.id,
        title: activity.title.clone(),
        started_at: activity.started_at,
        distance_meters: activity.distance_meters.map(round_metric),
        elevation_gain_meters: activity.elevation_gain_meters.map(round_metric),
        elapsed_seconds: activity_elapsed_seconds(activity, &samples),
    }
}

fn compare_metric<F>(
    key: &str,
    label: &str,
    unit: Option<&str>,
    direction: &str,
    analyses: &[CompareRideAnalysis],
    value_for: F,
) -> CompareRideMetricResponse
where
    F: Fn(&CompareRideAnalysis) -> Option<f64>,
{
    CompareRideMetricResponse {
        key: key.to_string(),
        label: label.to_string(),
        unit: unit.map(str::to_string),
        direction: direction.to_string(),
        values: analyses
            .iter()
            .map(|analysis| {
                let value = value_for(analysis).map(round_metric);
                CompareRideMetricValueResponse {
                    activity_id: analysis.activity_id,
                    value,
                    display: display_metric_value(value, unit),
                }
            })
            .collect(),
    }
}

fn display_metric_value(value: Option<f64>, unit: Option<&str>) -> String {
    match (value, unit) {
        (Some(value), Some("h")) => format!("{value:.1}h"),
        (Some(value), Some("%")) => format!("{value:.1}%"),
        (Some(value), Some(unit)) => format!("{value:.0} {unit}"),
        (Some(value), None) => format!("{value:.1}"),
        (None, _) => "n/a".to_string(),
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

fn activity_samples(activity: &activities::Model) -> Vec<TrendSample> {
    let derived_data = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    if derived_data.route_points.len() >= 2 {
        return derived_data
            .route_points
            .iter()
            .map(sample_from_route_point)
            .collect();
    }

    derived_data
        .chart_points
        .iter()
        .map(sample_from_chart_point)
        .collect()
}

fn activity_elapsed_seconds(activity: &activities::Model, samples: &[TrendSample]) -> i32 {
    activity
        .total_time_seconds
        .or_else(|| samples.last().map(|sample| sample.elapsed_seconds))
        .or_else(|| {
            activity
                .ended_at
                .map(|ended_at| (ended_at - activity.started_at).num_seconds() as i32)
        })
        .unwrap_or_default()
        .max(0)
}

fn sample_from_route_point(point: &ActivityRoutePoint) -> TrendSample {
    TrendSample {
        elapsed_seconds: point.elapsed_seconds,
        distance_meters: point.distance_meters,
        elevation_meters: point.elevation_meters,
        speed_mps: point.speed_mps,
        heart_rate_bpm: point.heart_rate_bpm,
        cadence_rpm: point.cadence_rpm,
        power_watts: point.power_watts,
    }
}

fn sample_from_chart_point(point: &ActivityChartPoint) -> TrendSample {
    TrendSample {
        elapsed_seconds: point.elapsed_seconds,
        distance_meters: point.distance_meters,
        elevation_meters: point.elevation_meters,
        speed_mps: point.speed_mps,
        heart_rate_bpm: point.heart_rate_bpm,
        cadence_rpm: point.cadence_rpm,
        power_watts: point.power_watts,
    }
}

fn segment_samples(
    samples: &[TrendSample],
    start_seconds: i32,
    end_seconds: i32,
) -> Vec<TrendSample> {
    samples
        .iter()
        .copied()
        .filter(|sample| {
            sample.elapsed_seconds >= start_seconds && sample.elapsed_seconds <= end_seconds
        })
        .collect()
}

fn hourly_durability(samples: &[TrendSample]) -> Vec<HourlyDurabilityResponse> {
    let Some(last) = samples.last() else {
        return Vec::new();
    };
    let hour_count = ((last.elapsed_seconds as f64) / 3600.0).ceil() as i32;
    let mut rows = Vec::new();

    for hour in 1..=hour_count.max(1) {
        let start = (hour - 1) * 3600;
        let end = (hour * 3600).min(last.elapsed_seconds);
        let segment = segment_samples(samples, start, end);
        if segment.len() < 2 {
            continue;
        }
        let distance = segment_distance(&segment);
        let average_speed = average_speed(&segment);
        let average_hr = average_heart_rate(&segment);
        let max_hr = segment
            .iter()
            .filter_map(|sample| sample.heart_rate_bpm)
            .max();
        let (moving_seconds, stopped_seconds, stop_count) = movement_breakdown(&segment);
        let ascent = positive_gain(&segment);

        rows.push(HourlyDurabilityResponse {
            hour,
            elapsed_start_seconds: start,
            elapsed_end_seconds: end,
            distance_meters: distance.map(round_metric),
            average_speed_mps: average_speed.map(round_metric),
            average_heart_rate_bpm: average_hr.map(round_metric),
            max_heart_rate_bpm: max_hr,
            ascent_meters: round_metric(ascent),
            moving_seconds,
            stopped_seconds,
            stop_count,
            efficiency_mps_per_bpm: efficiency(&segment).map(round_metric),
        });
    }

    rows
}

fn fatigue_start_hour(hourly: &[HourlyDurabilityResponse]) -> Option<i32> {
    let baseline = hourly
        .iter()
        .take(2)
        .filter_map(|row| row.efficiency_mps_per_bpm)
        .collect::<Vec<_>>();
    let baseline = median(baseline)?;

    hourly
        .iter()
        .find(|row| {
            row.hour >= 2
                && row
                    .efficiency_mps_per_bpm
                    .is_some_and(|efficiency| efficiency <= baseline * 0.90)
        })
        .map(|row| row.hour)
}

fn detect_climbs(activity: &activities::Model, samples: &[TrendSample]) -> Vec<ClimbResponse> {
    const MIN_GAIN_METERS: f64 = 20.0;
    const MIN_DURATION_SECONDS: i32 = 90;
    const MIN_DISTANCE_METERS: f64 = 300.0;
    const SUMMIT_CONFIRMATION_DROP_METERS: f64 = 5.0;
    const VALLEY_RESET_DROP_METERS: f64 = 8.0;

    let mut climbs = Vec::new();
    let total_elapsed = samples
        .last()
        .map(|sample| sample.elapsed_seconds)
        .unwrap_or_default();
    let Some((mut valley_index, mut valley_elevation)) = first_elevation_sample(samples) else {
        return climbs;
    };
    let mut crest_index = valley_index;
    let mut crest_elevation = valley_elevation;

    for index in (valley_index + 1)..samples.len() {
        let Some(elevation) = samples[index].elevation_meters else {
            continue;
        };

        if elevation < valley_elevation {
            let drop_from_crest = crest_elevation - elevation;
            if crest_index > valley_index && drop_from_crest >= SUMMIT_CONFIRMATION_DROP_METERS {
                finalize_climb(
                    activity,
                    samples,
                    valley_index,
                    crest_index,
                    crest_elevation - valley_elevation,
                    total_elapsed,
                    MIN_GAIN_METERS,
                    MIN_DURATION_SECONDS,
                    MIN_DISTANCE_METERS,
                    &mut climbs,
                );
                valley_index = index;
                valley_elevation = elevation;
                crest_index = index;
                crest_elevation = elevation;
                continue;
            }

            if crest_index == valley_index || drop_from_crest >= VALLEY_RESET_DROP_METERS {
                valley_index = index;
                valley_elevation = elevation;
                crest_index = index;
                crest_elevation = elevation;
            }
            continue;
        }

        if elevation > crest_elevation {
            crest_index = index;
            crest_elevation = elevation;
            continue;
        }

        let drop_from_crest = crest_elevation - elevation;
        if drop_from_crest < SUMMIT_CONFIRMATION_DROP_METERS {
            continue;
        }

        finalize_climb(
            activity,
            samples,
            valley_index,
            crest_index,
            crest_elevation - valley_elevation,
            total_elapsed,
            MIN_GAIN_METERS,
            MIN_DURATION_SECONDS,
            MIN_DISTANCE_METERS,
            &mut climbs,
        );
        valley_index = index;
        valley_elevation = elevation;
        crest_index = index;
        crest_elevation = elevation;
    }

    finalize_climb(
        activity,
        samples,
        valley_index,
        crest_index,
        crest_elevation - valley_elevation,
        total_elapsed,
        MIN_GAIN_METERS,
        MIN_DURATION_SECONDS,
        MIN_DISTANCE_METERS,
        &mut climbs,
    );

    climbs
}

fn finalize_climb(
    activity: &activities::Model,
    samples: &[TrendSample],
    start_index: usize,
    summit_index: usize,
    gain_meters: f64,
    total_elapsed_seconds: i32,
    min_gain_meters: f64,
    min_duration_seconds: i32,
    min_distance_meters: f64,
    climbs: &mut Vec<ClimbResponse>,
) {
    if summit_index <= start_index || summit_index >= samples.len() {
        return;
    }

    let segment = &samples[start_index..=summit_index];
    let duration_seconds =
        segment.last().unwrap().elapsed_seconds - segment.first().unwrap().elapsed_seconds;
    let distance_meters = segment_distance(segment).unwrap_or_default();

    if duration_seconds < min_duration_seconds
        || distance_meters < min_distance_meters
        || gain_meters < min_gain_meters
    {
        return;
    }

    let vertical_rate = 3600.0 * gain_meters / f64::from(duration_seconds.max(1));
    let summit_seconds = segment.last().unwrap().elapsed_seconds;

    climbs.push(ClimbResponse {
        activity_id: activity.id,
        activity_title: activity.title.clone(),
        climb_number: climbs.len() as i32 + 1,
        start_seconds: segment.first().unwrap().elapsed_seconds,
        summit_seconds,
        duration_seconds,
        distance_meters: round_metric(distance_meters),
        gain_meters: round_metric(gain_meters),
        average_grade_percent: Some(round_metric((gain_meters / distance_meters) * 100.0)),
        vertical_rate_meters_per_hour: round_metric(vertical_rate),
        average_speed_mps: average_speed(segment).map(round_metric),
        average_heart_rate_bpm: average_heart_rate(segment).map(round_metric),
        peak_heart_rate_bpm: segment
            .iter()
            .filter_map(|sample| sample.heart_rate_bpm)
            .max(),
        average_cadence_rpm: average_cadence(segment).map(round_metric),
        average_power_watts: average_power(segment).map(round_metric),
        heart_rate_recovery_30_seconds_bpm: heart_rate_recovery(samples, summit_index, 30),
        heart_rate_recovery_60_seconds_bpm: heart_rate_recovery(samples, summit_index, 60),
        seconds_to_drop_10_bpm: seconds_to_drop_bpm(samples, summit_index, 10),
        seconds_to_drop_15_bpm: seconds_to_drop_bpm(samples, summit_index, 15),
        summit_immediately_enters_descent: summit_enters_descent(samples, summit_index),
        first_or_second_half: if segment.last().unwrap().elapsed_seconds
            <= total_elapsed_seconds / 2
        {
            "first".to_string()
        } else {
            "second".to_string()
        },
    });
}

fn first_elevation_sample(samples: &[TrendSample]) -> Option<(usize, f64)> {
    samples
        .iter()
        .enumerate()
        .find_map(|(index, sample)| sample.elevation_meters.map(|elevation| (index, elevation)))
}

fn segment_distance(samples: &[TrendSample]) -> Option<f64> {
    let first = samples.iter().find_map(|sample| sample.distance_meters)?;
    let last = samples
        .iter()
        .rev()
        .find_map(|sample| sample.distance_meters)?;
    (last >= first).then_some(last - first)
}

fn average_speed(samples: &[TrendSample]) -> Option<f64> {
    let distance = segment_distance(samples)?;
    let duration = samples.last()?.elapsed_seconds - samples.first()?.elapsed_seconds;
    (duration > 0 && distance > 0.0).then_some(distance / f64::from(duration))
}

fn average_heart_rate(samples: &[TrendSample]) -> Option<f64> {
    let values = samples
        .iter()
        .filter_map(|sample| sample.heart_rate_bpm.map(f64::from))
        .collect::<Vec<_>>();
    mean(values)
}

fn average_cadence(samples: &[TrendSample]) -> Option<f64> {
    let values = samples
        .iter()
        .filter_map(|sample| sample.cadence_rpm.map(f64::from))
        .collect::<Vec<_>>();
    mean(values)
}

fn average_power(samples: &[TrendSample]) -> Option<f64> {
    let values = samples
        .iter()
        .filter_map(|sample| sample.power_watts.map(f64::from))
        .collect::<Vec<_>>();
    mean(values)
}

fn heart_rate_recovery(
    samples: &[TrendSample],
    summit_index: usize,
    recovery_seconds: i32,
) -> Option<i32> {
    let summit_heart_rate = samples.get(summit_index)?.heart_rate_bpm?;
    let target_seconds = samples.get(summit_index)?.elapsed_seconds + recovery_seconds;
    let recovery_heart_rate = heart_rate_at_or_after(samples, target_seconds)?;
    Some(summit_heart_rate - recovery_heart_rate)
}

fn heart_rate_at_or_after(samples: &[TrendSample], elapsed_seconds: i32) -> Option<i32> {
    samples
        .iter()
        .filter(|sample| sample.elapsed_seconds >= elapsed_seconds)
        .find_map(|sample| sample.heart_rate_bpm)
}

fn seconds_to_drop_bpm(samples: &[TrendSample], summit_index: usize, drop_bpm: i32) -> Option<i32> {
    let summit = samples.get(summit_index)?;
    let summit_heart_rate = summit.heart_rate_bpm?;
    let target_heart_rate = summit_heart_rate - drop_bpm;

    samples
        .iter()
        .skip(summit_index + 1)
        .find(|sample| {
            sample
                .heart_rate_bpm
                .is_some_and(|hr| hr <= target_heart_rate)
        })
        .map(|sample| sample.elapsed_seconds - summit.elapsed_seconds)
}

fn summit_enters_descent(samples: &[TrendSample], summit_index: usize) -> bool {
    let Some(summit) = samples.get(summit_index) else {
        return false;
    };
    let Some(summit_elevation) = summit.elevation_meters else {
        return false;
    };

    samples
        .iter()
        .skip(summit_index + 1)
        .take_while(|sample| sample.elapsed_seconds - summit.elapsed_seconds <= 120)
        .any(|sample| {
            sample
                .elevation_meters
                .is_some_and(|elevation| summit_elevation - elevation >= 5.0)
        })
}

fn efficiency(samples: &[TrendSample]) -> Option<f64> {
    let speed = average_speed(samples)?;
    let heart_rate = average_heart_rate(samples)?;
    (heart_rate > 0.0).then_some(speed / heart_rate)
}

fn late_ride_changes(samples: &[TrendSample]) -> (Option<f64>, Option<f64>) {
    let Some(last) = samples.last() else {
        return (None, None);
    };
    let total = last.elapsed_seconds;
    let early = segment_samples(samples, total / 10, (total as f64 * 0.35) as i32);
    let late = segment_samples(samples, (total as f64 * 0.75) as i32, total);

    (
        percent_change(average_speed(&early), average_speed(&late)),
        percent_change(average_heart_rate(&early), average_heart_rate(&late)),
    )
}

fn movement_breakdown(samples: &[TrendSample]) -> (i32, i32, i32) {
    let mut moving = 0;
    let mut stopped = 0;
    let mut stop_count = 0;
    let mut was_stopped = false;

    for window in samples.windows(2) {
        let previous = window[0];
        let current = window[1];
        let delta_time = current.elapsed_seconds - previous.elapsed_seconds;
        if !(1..=30).contains(&delta_time) {
            continue;
        }
        let speed = previous
            .speed_mps
            .or_else(|| {
                delta_distance(previous, current).map(|distance| distance / f64::from(delta_time))
            })
            .unwrap_or_default();
        if speed < 0.5 {
            stopped += delta_time;
            if !was_stopped {
                stop_count += 1;
            }
            was_stopped = true;
        } else {
            moving += delta_time;
            was_stopped = false;
        }
    }

    (moving, stopped, stop_count)
}

fn positive_gain(samples: &[TrendSample]) -> f64 {
    samples
        .windows(2)
        .filter_map(|window| delta_elevation(window[0], window[1]))
        .filter(|delta| *delta >= 1.0)
        .sum()
}

fn delta_distance(previous: TrendSample, current: TrendSample) -> Option<f64> {
    let delta = current.distance_meters? - previous.distance_meters?;
    (delta >= 0.0).then_some(delta)
}

fn delta_elevation(previous: TrendSample, current: TrendSample) -> Option<f64> {
    Some(current.elevation_meters? - previous.elevation_meters?)
}

fn percent_change(early: Option<f64>, late: Option<f64>) -> Option<f64> {
    let early = early?;
    let late = late?;
    (early.abs() > f64::EPSILON).then_some(((late - early) / early) * 100.0)
}

fn mean(values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len() % 2 == 0 {
        Some((values[middle - 1] + values[middle]) / 2.0)
    } else {
        Some(values[middle])
    }
}

fn max_by_metric<F>(climbs: &[ClimbResponse], metric: F) -> Option<ClimbResponse>
where
    F: Fn(&ClimbResponse) -> f64,
{
    climbs
        .iter()
        .cloned()
        .max_by(|a, b| metric(a).total_cmp(&metric(b)))
}

fn min_by_metric<F>(climbs: &[ClimbResponse], metric: F) -> Option<ClimbResponse>
where
    F: Fn(&ClimbResponse) -> f64,
{
    climbs
        .iter()
        .cloned()
        .min_by(|a, b| metric(a).total_cmp(&metric(b)))
}

fn percentile_by_metric<F>(
    climbs: &[ClimbResponse],
    percentile: f64,
    metric: F,
) -> Option<ClimbResponse>
where
    F: Fn(&ClimbResponse) -> f64,
{
    if climbs.is_empty() {
        return None;
    }
    let mut sorted = climbs.to_vec();
    sorted.sort_by(|a, b| metric(a).total_cmp(&metric(b)));
    let index = ((sorted.len() - 1) as f64 * percentile).round() as usize;
    sorted.get(index).cloned()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_activity() -> activities::Model {
        let now = Utc::now();
        activities::Model {
            id: 42,
            user_id: 7,
            activity_import_id: None,
            title: "Benchmark climb".to_string(),
            sport: "Ride".to_string(),
            source: "test".to_string(),
            source_correlation_id: None,
            original_filename: None,
            format: None,
            activity_type: "ride".to_string(),
            started_at: now,
            ended_at: None,
            distance_meters: None,
            moving_time_seconds: None,
            total_time_seconds: None,
            elevation_gain_meters: None,
            elevation_loss_meters: None,
            average_speed_mps: None,
            max_speed_mps: None,
            average_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            average_cadence_rpm: None,
            max_cadence_rpm: None,
            calories: None,
            estimated_ftp_watts: None,
            heart_rate_zones_json: None,
            derived_data_json: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn sample(
        elapsed_seconds: i32,
        distance_meters: f64,
        elevation_meters: f64,
        heart_rate_bpm: i32,
    ) -> TrendSample {
        TrendSample {
            elapsed_seconds,
            distance_meters: Some(distance_meters),
            elevation_meters: Some(elevation_meters),
            speed_mps: None,
            heart_rate_bpm: Some(heart_rate_bpm),
            cadence_rpm: Some(80),
            power_watts: Some(220),
        }
    }

    #[test]
    fn detects_valley_to_confirmed_crest_climb_with_recovery_metrics() {
        let samples = vec![
            sample(0, 0.0, 100.0, 120),
            sample(30, 125.0, 106.0, 130),
            sample(60, 250.0, 114.0, 142),
            sample(90, 390.0, 122.0, 152),
            sample(120, 520.0, 126.0, 160),
            sample(150, 560.0, 123.0, 154),
            sample(160, 590.0, 120.0, 150),
            sample(180, 640.0, 118.0, 142),
        ];

        let climbs = detect_climbs(&test_activity(), &samples);

        assert_eq!(climbs.len(), 1);
        let climb = &climbs[0];
        assert_eq!(climb.start_seconds, 0);
        assert_eq!(climb.summit_seconds, 120);
        assert_eq!(climb.duration_seconds, 120);
        assert_eq!(climb.distance_meters, 520.0);
        assert_eq!(climb.gain_meters, 26.0);
        assert_eq!(climb.average_cadence_rpm, Some(80.0));
        assert_eq!(climb.average_power_watts, Some(220.0));
        assert_eq!(climb.heart_rate_recovery_30_seconds_bpm, Some(6));
        assert_eq!(climb.heart_rate_recovery_60_seconds_bpm, Some(18));
        assert_eq!(climb.seconds_to_drop_10_bpm, Some(40));
        assert_eq!(climb.seconds_to_drop_15_bpm, Some(60));
        assert!(climb.summit_immediately_enters_descent);
    }

    #[test]
    fn rejects_short_or_unconfirmed_climbs_below_defaults() {
        let short_samples = vec![
            sample(0, 0.0, 100.0, 120),
            sample(30, 120.0, 110.0, 130),
            sample(60, 240.0, 124.0, 145),
            sample(80, 260.0, 118.0, 140),
        ];

        assert!(detect_climbs(&test_activity(), &short_samples).is_empty());
    }
}
