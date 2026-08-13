use crate::activity_details::{
    deserialize_derived_activity_data, ActivityChartPoint, ActivityRoutePoint,
};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{
    activities, activity_training_analyses, fitness_freshness_daily, user_preferences,
};
use crate::storage::AppStorage;
use crate::training_profile::deserialize_activity_heart_rate_zones;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, Select};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

const FEET_PER_METER: f64 = 3.28084;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportId {
    RideSummary,
    Endurance,
    Climbing,
    Fatigue,
    CompareRides,
    Reassessment,
    AggregateTrends,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportFilterKey {
    ActivityIds,
    MinDuration,
    MinDistance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportMetricDirection {
    Higher,
    Lower,
    Neutral,
}

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
    pub min_duration_seconds: Option<i32>,
    pub min_distance_meters: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingReportDefinitionsResponse {
    pub reports: Vec<TrainingReportDefinitionResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingReportDefinitionResponse {
    pub id: ReportId,
    pub display_name: String,
    pub short_purpose: String,
    pub supported_filters: Vec<ReportFilterKey>,
    pub required_data_quality: Vec<String>,
    pub result_sections: Vec<String>,
    pub metrics: Vec<TrainingReportMetricDefinitionResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingReportMetricDefinitionResponse {
    pub key: String,
    pub label: String,
    pub unit: Option<String>,
    pub direction: ReportMetricDirection,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingReportPointResponse {
    pub bucket_start: String,
    pub bucket_end: String,
    pub z2_average_speed_mps: Option<f64>,
    pub average_aerobic_decoupling_percent: Option<f64>,
    pub climbing_pace_feet_per_week: Option<f64>,
    pub climbing_vertical_rate_feet_per_hour: Option<f64>,
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
    pub reassessment: Option<ReassessmentReportResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentReportResponse {
    pub verdict: ReassessmentVerdict,
    pub verdict_title: String,
    pub verdict_detail: String,
    pub target: ReassessmentTargetResponse,
    pub ability_estimate: ReassessmentAbilityEstimateResponse,
    pub recent_window: ReassessmentWindowResponse,
    pub spring_baseline_window: ReassessmentWindowResponse,
    pub improvement: ReassessmentImprovementResponse,
    pub endurance_progression: ReassessmentSignalResponse,
    pub climbing_density: ReassessmentSignalResponse,
    pub long_ride_pace: ReassessmentSignalResponse,
    pub fitness_delta: ReassessmentSignalResponse,
    pub benchmark_rides: Vec<ReassessmentBenchmarkRideResponse>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReassessmentVerdict {
    OnTrack,
    PlausibleButRisky,
    NeedsMoreEvidence,
    MissingData,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentTargetResponse {
    pub event_name: String,
    pub target_source: ReassessmentTargetSource,
    pub target_source_detail: String,
    pub target_date: Option<String>,
    pub event_profile: Option<String>,
    pub target_finish_seconds: Option<i32>,
    pub target_distance_meters: Option<f64>,
    pub target_elevation_gain_meters: Option<f64>,
    pub target_speed_mps: Option<f64>,
    pub target_speed_mph: Option<f64>,
    pub target_climb_density_feet_per_hour: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentAbilityEstimateResponse {
    pub estimated_finish_seconds: Option<i32>,
    pub estimated_speed_mph: Option<f64>,
    pub pace_limited_finish_seconds: Option<i32>,
    pub climbing_limited_finish_seconds: Option<i32>,
    pub current_long_ride_speed_mph: Option<f64>,
    pub current_climb_density_feet_per_hour: Option<f64>,
    pub limiter: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReassessmentTargetSource {
    SavedGoal,
    MissingGoal,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentWindowResponse {
    pub label: String,
    pub start_date: String,
    pub end_date: String,
    pub activity_count: i32,
    pub long_ride_count: i32,
    pub total_distance_miles: f64,
    pub total_elevation_gain_feet: f64,
    pub best_long_ride_distance_miles: Option<f64>,
    pub best_long_ride_duration_seconds: Option<i32>,
    pub best_long_ride_speed_mph: Option<f64>,
    pub best_long_ride_climbing_density_feet_per_hour: Option<f64>,
    pub aggregate_long_ride_climbing_density_feet_per_hour: Option<f64>,
    pub median_long_ride_decoupling_percent: Option<f64>,
    pub median_long_ride_late_speed_change_percent: Option<f64>,
    pub median_long_ride_fatigue_index: Option<f64>,
    pub average_fitness: Option<f64>,
    pub latest_fitness: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentImprovementResponse {
    pub fitness_change: Option<f64>,
    pub fitness_change_percent: Option<f64>,
    pub long_ride_speed_change_mph: Option<f64>,
    pub long_ride_speed_change_percent: Option<f64>,
    pub long_ride_distance_change_miles: Option<f64>,
    pub long_ride_distance_change_percent: Option<f64>,
    pub climbing_density_change_feet_per_hour: Option<f64>,
    pub climbing_density_change_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentSignalResponse {
    pub status: ReassessmentVerdict,
    pub title: String,
    pub detail: String,
    pub current_value: Option<f64>,
    pub projected_current_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub target_value: Option<f64>,
    pub unit: String,
    pub current_source_activity_id: Option<i32>,
    pub current_source_title: Option<String>,
    pub current_source_started_at: Option<DateTime<Utc>>,
    pub last_known_value: Option<f64>,
    pub last_known_source_activity_id: Option<i32>,
    pub last_known_source_title: Option<String>,
    pub last_known_source_started_at: Option<DateTime<Utc>>,
    pub last_known_days_old: Option<i64>,
    pub projection_detail: Option<String>,
    pub baseline_source_activity_id: Option<i32>,
    pub baseline_source_title: Option<String>,
    pub baseline_source_started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ReassessmentBenchmarkRideResponse {
    pub activity_id: i32,
    pub title: String,
    pub started_at: DateTime<Utc>,
    pub distance_miles: Option<f64>,
    pub elevation_gain_feet: Option<f64>,
    pub elapsed_seconds: i32,
    pub moving_seconds: Option<i32>,
    pub elapsed_speed_mph: Option<f64>,
    pub moving_speed_mph: Option<f64>,
    pub climbing_density_feet_per_hour: Option<f64>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub late_speed_change_percent: Option<f64>,
    pub fatigue_index: Option<f64>,
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
    pub median_fatigue_index: Option<f64>,
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
    pub fatigue_index: Option<f64>,
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
    pub worst_fatigue_index: Option<f64>,
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
    pub climb_rate_meters_per_hour: Option<f64>,
    pub moving_seconds: i32,
    pub stopped_seconds: i32,
    pub stop_count: i32,
    pub stop_frequency_per_hour: f64,
    pub efficiency_mps_per_bpm: Option<f64>,
    pub fatigue_index: Option<f64>,
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
    pub trend: Option<CompareRideMetricTrendResponse>,
    pub values: Vec<CompareRideMetricValueResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct CompareRideMetricTrendResponse {
    pub first_activity_id: i32,
    pub latest_activity_id: i32,
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
    pub display: String,
    pub interpretation: String,
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
    climb_rate_sum: f64,
    climb_rate_count: i32,
    elevation_meters_total: f64,
    zone_seconds: [i32; 5],
}

#[utoipa::path(
    get,
    path = "/api/training/reports/definitions",
    responses(
        (status = 200, description = "Training report definitions", body = TrainingReportDefinitionsResponse),
        (status = 401, description = "Not authenticated"),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_training_report_definitions(
    UserContext { .. }: UserContext<AppStorage>,
) -> Json<TrainingReportDefinitionsResponse> {
    Json(TrainingReportDefinitionsResponse {
        reports: report_definitions(),
    })
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
    let report_id = parse_report_id(query.report.as_deref())?;
    validate_report_filters(&query)?;

    let activity_models = filter_activities(
        activities::Entity::find()
            .filter(activities::Column::UserId.eq(user.id))
            .filter(activities::Column::StartedAt.gte(range_start))
            .filter(activities::Column::StartedAt.lte(range_end)),
        &query,
    )
    .all(&state.db)
    .await?;

    match report_id {
        ReportId::RideSummary => {
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
                reassessment: None,
            }));
        }
        ReportId::Endurance => {
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
                reassessment: None,
            }));
        }
        ReportId::Climbing => {
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
                reassessment: None,
            }));
        }
        ReportId::Fatigue => {
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
                reassessment: None,
            }));
        }
        ReportId::CompareRides => {
            let selected_ids = parse_activity_ids(query.activity_ids.as_deref())?;
            let selected_models = if selected_ids.is_empty() {
                Vec::new()
            } else {
                filter_activities(
                    activities::Entity::find()
                        .filter(activities::Column::UserId.eq(user.id))
                        .filter(activities::Column::Id.is_in(selected_ids)),
                    &query,
                )
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
                reassessment: None,
            }));
        }
        ReportId::Reassessment => {
            let preferences = user_preferences::Entity::find()
                .filter(user_preferences::Column::UserId.eq(user.id))
                .one(&state.db)
                .await?;
            let reassessment_range_end = make_utc_datetime(
                now.date_naive().year(),
                now.date_naive().month(),
                now.date_naive().day(),
                23,
                59,
                59,
            )?;
            let reassessment_range_start = match preferences
                .as_ref()
                .and_then(|model| model.xc_goal_start_date)
            {
                Some(start_date) => make_utc_datetime(
                    start_date.year(),
                    start_date.month(),
                    start_date.day(),
                    0,
                    0,
                    0,
                )?,
                None => range_start,
            };
            let spring_year = if reassessment_range_end.date_naive().month() < 6 {
                reassessment_range_end.date_naive().year() - 1
            } else {
                reassessment_range_end.date_naive().year()
            };
            let spring_start_date = NaiveDate::from_ymd_opt(spring_year, 3, 1)
                .ok_or_else(|| AppError::bad_request("Invalid spring baseline date"))?;
            let spring_start = make_utc_datetime(
                spring_start_date.year(),
                spring_start_date.month(),
                spring_start_date.day(),
                0,
                0,
                0,
            )?;
            let analysis_range_start = reassessment_range_start.min(spring_start);
            let reassessment_activity_models = filter_activities(
                activities::Entity::find()
                    .filter(activities::Column::UserId.eq(user.id))
                    .filter(activities::Column::StartedAt.gte(analysis_range_start))
                    .filter(activities::Column::StartedAt.lte(reassessment_range_end)),
                &query,
            )
            .all(&state.db)
            .await?;
            let fitness_rows = fitness_freshness_daily::Entity::find()
                .filter(fitness_freshness_daily::Column::UserId.eq(user.id))
                .filter(fitness_freshness_daily::Column::Day.gte(analysis_range_start.date_naive()))
                .filter(
                    fitness_freshness_daily::Column::Day.lte(reassessment_range_end.date_naive()),
                )
                .order_by_asc(fitness_freshness_daily::Column::Day)
                .all(&state.db)
                .await?;

            return Ok(Json(TrainingReportsResponse {
                generated_at: now,
                boundary,
                range_start: reassessment_range_start.to_rfc3339(),
                range_end: reassessment_range_end.to_rfc3339(),
                points: Vec::new(),
                ride_summary: None,
                endurance: None,
                climbing: None,
                fatigue: None,
                compare_rides: None,
                reassessment: Some(build_reassessment_report(
                    &reassessment_activity_models,
                    &fitness_rows,
                    preferences.as_ref(),
                    reassessment_range_start.date_naive(),
                    reassessment_range_end.date_naive(),
                )),
            }));
        }
        ReportId::AggregateTrends => {}
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
        let standalone_analysis = compare_analysis_from_activity(activity);

        if let Some(speed_mps) = analysis
            .and_then(|model| model.z2_average_speed_mps)
            .or(standalone_analysis.z2_average_speed_mps)
        {
            bucket.z2_speed_sum += speed_mps;
            bucket.z2_speed_count += 1;
        }

        if let Some(decoupling_percent) = analysis
            .and_then(|model| model.aerobic_decoupling_percent)
            .or(standalone_analysis.aerobic_decoupling_percent)
        {
            bucket.decoupling_sum += decoupling_percent;
            bucket.decoupling_count += 1;
        }

        if let Some(climb_rate_meters_per_hour) = standalone_analysis.median_climb_rate {
            bucket.climb_rate_sum += climb_rate_meters_per_hour * FEET_PER_METER;
            bucket.climb_rate_count += 1;
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
            climbing_vertical_rate_feet_per_hour: average_or_none(
                accumulator.climb_rate_sum,
                accumulator.climb_rate_count,
            ),
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
        reassessment: None,
    }))
}

fn report_definitions() -> Vec<TrainingReportDefinitionResponse> {
    vec![
        TrainingReportDefinitionResponse {
            id: ReportId::RideSummary,
            display_name: "Ride Summary".to_string(),
            short_purpose: "Overall volume, intensity, climbing, stopped time, and data quality."
                .to_string(),
            supported_filters: vec![ReportFilterKey::MinDuration, ReportFilterKey::MinDistance],
            required_data_quality: vec![
                "distance".to_string(),
                "elevation".to_string(),
                "time".to_string(),
                "heart_rate".to_string(),
            ],
            result_sections: vec![
                "summary_cards".to_string(),
                "heart_rate_zones".to_string(),
                "data_quality".to_string(),
            ],
            metrics: vec![
                metric_definition(
                    "distance",
                    "Distance",
                    Some("mi"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "elevation_gain",
                    "Elevation",
                    Some("ft"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "moving_time",
                    "Moving Time",
                    Some("seconds"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "stopped_time",
                    "Stopped Time",
                    Some("seconds"),
                    ReportMetricDirection::Lower,
                ),
                metric_definition(
                    "heart_rate_zones",
                    "Heart Rate Zones",
                    None,
                    ReportMetricDirection::Neutral,
                ),
            ],
        },
        TrainingReportDefinitionResponse {
            id: ReportId::Endurance,
            display_name: "Endurance".to_string(),
            short_purpose: "Aerobic durability, efficiency, speed, HR, and late-ride drift."
                .to_string(),
            supported_filters: vec![ReportFilterKey::MinDuration, ReportFilterKey::MinDistance],
            required_data_quality: vec![
                "distance".to_string(),
                "time".to_string(),
                "heart_rate".to_string(),
            ],
            result_sections: vec!["summary_cards".to_string(), "ride_table".to_string()],
            metrics: vec![
                metric_definition(
                    "aerobic_decoupling_percent",
                    "Aerobic Decoupling",
                    Some("%"),
                    ReportMetricDirection::Lower,
                ),
                metric_definition(
                    "hourly_efficiency",
                    "Hourly Efficiency",
                    Some("mps_per_bpm"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "late_speed_change_percent",
                    "Late Ride Fade",
                    Some("%"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "late_heart_rate_change_percent",
                    "Late HR Change",
                    Some("%"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "fatigue_index",
                    "Fatigue Index",
                    Some("score"),
                    ReportMetricDirection::Lower,
                ),
            ],
        },
        TrainingReportDefinitionResponse {
            id: ReportId::Climbing,
            display_name: "Climbing".to_string(),
            short_purpose: "Climb summaries, vertical rate trends, and raw climb rows.".to_string(),
            supported_filters: vec![ReportFilterKey::MinDuration, ReportFilterKey::MinDistance],
            required_data_quality: vec![
                "distance".to_string(),
                "elevation".to_string(),
                "time".to_string(),
            ],
            result_sections: vec!["summary_cards".to_string(), "climb_table".to_string()],
            metrics: vec![
                metric_definition(
                    "longest_climb",
                    "Longest Climb",
                    Some("seconds"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "median_climb_rate",
                    "Median Climb Rate",
                    Some("m/h"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "heart_rate_recovery_60_seconds_bpm",
                    "60s HR Recovery",
                    Some("bpm"),
                    ReportMetricDirection::Higher,
                ),
            ],
        },
        TrainingReportDefinitionResponse {
            id: ReportId::Fatigue,
            display_name: "Fatigue".to_string(),
            short_purpose: "Hour-by-hour ride fade across HR, speed, climbing, and stops."
                .to_string(),
            supported_filters: vec![ReportFilterKey::MinDuration, ReportFilterKey::MinDistance],
            required_data_quality: vec![
                "distance".to_string(),
                "time".to_string(),
                "heart_rate".to_string(),
            ],
            result_sections: vec!["ride_sections".to_string(), "hourly_table".to_string()],
            metrics: vec![
                metric_definition(
                    "average_speed",
                    "Hourly Speed",
                    Some("m/s"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "average_hr",
                    "Hourly HR",
                    Some("bpm"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "stop_count",
                    "Stop Frequency",
                    Some("stops/hour"),
                    ReportMetricDirection::Lower,
                ),
                metric_definition(
                    "climb_rate",
                    "Climb Rate",
                    Some("m/h"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "efficiency",
                    "Efficiency",
                    Some("mps_per_bpm"),
                    ReportMetricDirection::Higher,
                ),
            ],
        },
        TrainingReportDefinitionResponse {
            id: ReportId::CompareRides,
            display_name: "Compare Rides".to_string(),
            short_purpose: "Side-by-side comparison of selected races and benchmark rides."
                .to_string(),
            supported_filters: vec![
                ReportFilterKey::ActivityIds,
                ReportFilterKey::MinDuration,
                ReportFilterKey::MinDistance,
            ],
            required_data_quality: vec![
                "distance".to_string(),
                "elevation".to_string(),
                "time".to_string(),
                "heart_rate".to_string(),
            ],
            result_sections: vec![
                "comparison_table".to_string(),
                "candidate_table".to_string(),
            ],
            metrics: vec![
                metric_definition(
                    "aerobic_decoupling_percent",
                    "Aerobic Decoupling",
                    Some("%"),
                    ReportMetricDirection::Lower,
                ),
                metric_definition(
                    "median_climb_rate",
                    "Median Climb Rate",
                    Some("m/h"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "median_60s_hr_recovery_bpm",
                    "Median 60s HR Recovery",
                    Some("bpm"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "late_speed_change_percent",
                    "Late Ride Fade",
                    Some("%"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "stopped_time_percent",
                    "Stopped Time",
                    Some("%"),
                    ReportMetricDirection::Lower,
                ),
                metric_definition(
                    "moving_speed_mph",
                    "Moving Speed",
                    Some("mph"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "z2_average_speed_mph",
                    "Z2 Speed",
                    Some("mph"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "z2_time",
                    "Z2 Time",
                    Some("hours"),
                    ReportMetricDirection::Neutral,
                ),
            ],
        },
        TrainingReportDefinitionResponse {
            id: ReportId::Reassessment,
            display_name: "Reassessment".to_string(),
            short_purpose: "Reassess the active XC event goal from endurance progression, climbing density, elapsed long-ride pace, and spring-baseline fitness delta.".to_string(),
            supported_filters: vec![ReportFilterKey::MinDuration, ReportFilterKey::MinDistance],
            required_data_quality: vec![
                "distance".to_string(),
                "elevation".to_string(),
                "time".to_string(),
                "heart_rate".to_string(),
                "fitness_freshness".to_string(),
            ],
            result_sections: vec![
                "verdict".to_string(),
                "readiness_signals".to_string(),
                "spring_vs_recent".to_string(),
                "benchmark_rides".to_string(),
            ],
            metrics: vec![
                metric_definition(
                    "endurance_progression",
                    "Endurance Progression",
                    None,
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "climbing_density",
                    "Climbing Density",
                    Some("ft/h"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "long_ride_pace",
                    "Elapsed Long-Ride Pace",
                    Some("mph"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "fitness_delta",
                    "Fitness Delta",
                    Some("CTL"),
                    ReportMetricDirection::Higher,
                ),
            ],
        },
        TrainingReportDefinitionResponse {
            id: ReportId::AggregateTrends,
            display_name: "Aggregate Trends".to_string(),
            short_purpose: "Existing weekly, monthly, zone, climbing, and elevation charts."
                .to_string(),
            supported_filters: vec![ReportFilterKey::MinDuration, ReportFilterKey::MinDistance],
            required_data_quality: vec![
                "distance".to_string(),
                "elevation".to_string(),
                "heart_rate_zones".to_string(),
            ],
            result_sections: vec!["bucket_charts".to_string()],
            metrics: vec![
                metric_definition(
                    "z2_speed",
                    "Z2 Speed",
                    Some("m/s"),
                    ReportMetricDirection::Neutral,
                ),
                metric_definition(
                    "aerobic_decoupling_percent",
                    "Decoupling",
                    Some("%"),
                    ReportMetricDirection::Lower,
                ),
                metric_definition(
                    "climbing_pace",
                    "Median Climb Rate",
                    Some("ft/h"),
                    ReportMetricDirection::Higher,
                ),
                metric_definition(
                    "heart_rate_zones",
                    "Heart Rate Zones",
                    None,
                    ReportMetricDirection::Neutral,
                ),
            ],
        },
    ]
}

fn metric_definition(
    key: &str,
    label: &str,
    unit: Option<&str>,
    direction: ReportMetricDirection,
) -> TrainingReportMetricDefinitionResponse {
    TrainingReportMetricDefinitionResponse {
        key: key.to_string(),
        label: label.to_string(),
        unit: unit.map(str::to_string),
        direction,
    }
}

fn parse_report_id(raw: Option<&str>) -> Result<ReportId, AppError> {
    match raw {
        None | Some("") | Some("aggregate_trends") => Ok(ReportId::AggregateTrends),
        Some("ride_summary") => Ok(ReportId::RideSummary),
        Some("endurance") => Ok(ReportId::Endurance),
        Some("climbing") => Ok(ReportId::Climbing),
        Some("fatigue") => Ok(ReportId::Fatigue),
        Some("compare_rides") => Ok(ReportId::CompareRides),
        Some("reassessment") => Ok(ReportId::Reassessment),
        Some(_) => Err(AppError::bad_request("Unknown report id")),
    }
}

fn validate_report_filters(query: &TrainingReportsQuery) -> Result<(), AppError> {
    if query
        .min_duration_seconds
        .is_some_and(|seconds| seconds < 0)
    {
        return Err(AppError::bad_request(
            "min_duration_seconds must be greater than or equal to zero",
        ));
    }

    if query
        .min_distance_meters
        .is_some_and(|meters| meters < 0.0 || !meters.is_finite())
    {
        return Err(AppError::bad_request(
            "min_distance_meters must be a finite non-negative number",
        ));
    }

    Ok(())
}

fn filter_activities(
    mut query_builder: Select<activities::Entity>,
    query: &TrainingReportsQuery,
) -> Select<activities::Entity> {
    if let Some(min_duration_seconds) = query
        .min_duration_seconds
        .filter(|min_duration_seconds| *min_duration_seconds > 0)
    {
        query_builder = query_builder.filter(
            Condition::any()
                .add(activities::Column::MovingTimeSeconds.gte(min_duration_seconds))
                .add(activities::Column::TotalTimeSeconds.gte(min_duration_seconds)),
        );
    }

    if let Some(min_distance_meters) = query
        .min_distance_meters
        .filter(|min_distance_meters| *min_distance_meters > 0.0)
    {
        query_builder =
            query_builder.filter(activities::Column::DistanceMeters.gte(min_distance_meters));
    }

    query_builder
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
            let hourly = hourly_durability(&samples);
            let fatigue_index = worst_fatigue_index(&hourly);

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
                fatigue_index,
                hourly,
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
        median_fatigue_index: median(rides.iter().filter_map(|ride| ride.fatigue_index).collect())
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
                worst_fatigue_index: worst_fatigue_index(&hourly),
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

    let mut selected_sorted = selected.iter().collect::<Vec<_>>();
    selected_sorted.sort_by(|a, b| a.started_at.cmp(&b.started_at));

    let selected_rides = selected_sorted
        .iter()
        .map(|activity| compare_column_from_activity(activity))
        .collect::<Vec<_>>();

    let analyses = selected_sorted
        .iter()
        .map(|activity| compare_analysis_from_activity(activity))
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
            "moving_speed_mph",
            "Moving Speed",
            Some("mph"),
            "neutral",
            &analyses,
            |analysis| analysis.moving_speed_mps.map(|value| value * 2.236_936),
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
            "median_60s_hr_recovery_bpm",
            "Median 60s HR Recovery",
            Some("bpm"),
            "higher",
            &analyses,
            |analysis| analysis.median_60s_hr_recovery_bpm,
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
            "z2_average_speed_mph",
            "Z2 Speed",
            Some("mph"),
            "higher",
            &analyses,
            |analysis| analysis.z2_average_speed_mps.map(|value| value * 2.236_936),
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

const REASSESSMENT_LONG_RIDE_MIN_SECONDS: i32 = 10_800;
const REASSESSMENT_LONG_RIDE_MIN_DISTANCE_METERS: f64 = 56_327.04;
const REASSESSMENT_LONG_RIDE_TARGET_RATIO: f64 = 0.65;

#[derive(Debug, Clone)]
struct ReassessmentTarget {
    response: ReassessmentTargetResponse,
}

#[derive(Debug, Clone)]
struct ReassessmentWindowMetrics {
    response: ReassessmentWindowResponse,
    benchmark_rides: Vec<ReassessmentBenchmarkRideResponse>,
    endurance_ride: Option<ReassessmentBenchmarkRideResponse>,
    pace_ride: Option<ReassessmentBenchmarkRideResponse>,
}

fn build_reassessment_report(
    activities: &[activities::Model],
    fitness_rows: &[fitness_freshness_daily::Model],
    preferences: Option<&user_preferences::Model>,
    range_start: NaiveDate,
    range_end: NaiveDate,
) -> ReassessmentReportResponse {
    let target = reassessment_target(preferences);
    let spring_year = if range_end.month() < 6 {
        range_end.year() - 1
    } else {
        range_end.year()
    };
    let spring_start = NaiveDate::from_ymd_opt(spring_year, 3, 1).unwrap_or(range_end);
    let spring_end = NaiveDate::from_ymd_opt(spring_year, 5, 31).unwrap_or(range_end);
    let training_start = range_start.min(range_end);
    let current_start = if training_start <= spring_end {
        (spring_end + Duration::days(1)).min(range_end)
    } else {
        training_start
    };
    let prior_training = if training_start < current_start {
        Some(reassessment_window_metrics(
            "Last known training evidence",
            training_start,
            current_start - Duration::days(1),
            activities,
            fitness_rows,
        ))
    } else {
        None
    };

    let recent = reassessment_window_metrics(
        "Current training block",
        current_start,
        range_end,
        activities,
        fitness_rows,
    );
    let spring = reassessment_window_metrics(
        "Spring baseline",
        spring_start,
        spring_end,
        activities,
        fitness_rows,
    );
    let improvement = reassessment_improvement(&recent.response, &spring.response);
    let ability_estimate = reassessment_ability_estimate(&recent, &target);

    let endurance_progression = reassessment_endurance_signal(
        &recent,
        &spring,
        prior_training.as_ref(),
        &target,
        fitness_rows,
        range_end,
    );
    let climbing_density = reassessment_climbing_density_signal(
        &recent,
        &spring,
        prior_training.as_ref(),
        &target,
        fitness_rows,
        range_end,
    );
    let long_ride_pace = reassessment_long_ride_pace_signal(
        &recent,
        &spring,
        prior_training.as_ref(),
        &target,
        fitness_rows,
        range_end,
    );
    let fitness_delta =
        reassessment_fitness_delta_signal(&recent.response, &spring.response, range_end);
    let verdict = reassessment_overall_verdict(&[
        endurance_progression.status,
        climbing_density.status,
        long_ride_pace.status,
        fitness_delta.status,
    ]);
    let (verdict_title, verdict_detail) = reassessment_verdict_copy(
        verdict,
        &target.response,
        &recent.response,
        &spring.response,
        &fitness_delta,
        &ability_estimate,
    );

    let mut benchmark_rides = recent
        .benchmark_rides
        .iter()
        .chain(spring.benchmark_rides.iter())
        .cloned()
        .collect::<Vec<_>>();
    benchmark_rides.sort_by(|a, b| {
        b.elapsed_seconds
            .cmp(&a.elapsed_seconds)
            .then_with(|| b.started_at.cmp(&a.started_at))
    });
    benchmark_rides.truncate(8);

    let mut notes = vec![
        "The saved training start anchors this report; current evidence starts after the spring baseline when those windows overlap.".to_string(),
        "Long rides are rides at least 3 hours or 35 miles.".to_string(),
        "Long-ride pace uses elapsed time because the event clock does not pause for breaks."
            .to_string(),
        "Moving speed remains useful context, but it is not used for the pace verdict.".to_string(),
    ];
    if target.response.target_source != ReassessmentTargetSource::SavedGoal {
        notes.push(target.response.target_source_detail.clone());
    }
    if target.response.target_finish_seconds.is_none() {
        notes.push("The active XC goal does not include a target finish time, so target pace and target climbing-density signals are limited.".to_string());
    }
    if spring.response.activity_count == 0 {
        notes.push(
            "No spring-baseline rides were found, so improvement claims are limited.".to_string(),
        );
    }
    if recent.response.long_ride_count == 0 {
        notes.push("No current long rides were found in the reassessment window.".to_string());
    }

    ReassessmentReportResponse {
        verdict,
        verdict_title,
        verdict_detail,
        target: target.response,
        ability_estimate,
        recent_window: recent.response,
        spring_baseline_window: spring.response,
        improvement,
        endurance_progression,
        climbing_density,
        long_ride_pace,
        fitness_delta,
        benchmark_rides,
        notes,
    }
}

fn reassessment_target(preferences: Option<&user_preferences::Model>) -> ReassessmentTarget {
    if let Some(preferences) = preferences {
        if let (Some(distance), Some(elevation)) = (
            preferences.xc_goal_target_distance_meters,
            preferences.xc_goal_target_elevation_gain_meters,
        ) {
            if distance > 0.0 && elevation > 0.0 {
                let target_finish_seconds = preferences.xc_goal_target_finish_time_seconds;
                let target_speed_mps = target_finish_seconds
                    .and_then(|seconds| (seconds > 0).then_some(distance / f64::from(seconds)));
                let target_climb_density_feet_per_hour =
                    target_finish_seconds.and_then(|seconds| {
                        (seconds > 0)
                            .then_some(elevation * FEET_PER_METER / (f64::from(seconds) / 3600.0))
                    });

                return ReassessmentTarget {
                    response: ReassessmentTargetResponse {
                        event_name: preferences
                            .xc_goal_event_name
                            .clone()
                            .unwrap_or_else(|| "XC event goal".to_string()),
                        target_source: ReassessmentTargetSource::SavedGoal,
                        target_source_detail:
                            "Using the saved XC event goal from account preferences.".to_string(),
                        target_date: preferences
                            .xc_goal_target_date
                            .map(|date| date.format("%Y-%m-%d").to_string()),
                        event_profile: preferences.xc_goal_event_profile.clone(),
                        target_finish_seconds,
                        target_distance_meters: Some(round_metric(distance)),
                        target_elevation_gain_meters: Some(round_metric(elevation)),
                        target_speed_mps: target_speed_mps.map(round_metric),
                        target_speed_mph: target_speed_mps
                            .map(|speed| round_metric(speed * 2.236_936)),
                        target_climb_density_feet_per_hour: target_climb_density_feet_per_hour
                            .map(round_metric),
                    },
                };
            }
        }
    }

    ReassessmentTarget {
        response: ReassessmentTargetResponse {
            event_name: "XC event goal missing".to_string(),
            target_source: ReassessmentTargetSource::MissingGoal,
            target_source_detail: if preferences.is_some() {
                "The saved XC goal is missing distance or elevation, so target-specific pace and climbing-density signals are unavailable."
            } else {
                "No saved XC goal was found, so target-specific pace and climbing-density signals are unavailable."
            }
            .to_string(),
            target_date: None,
            event_profile: None,
            target_finish_seconds: None,
            target_distance_meters: None,
            target_elevation_gain_meters: None,
            target_speed_mps: None,
            target_speed_mph: None,
            target_climb_density_feet_per_hour: None,
        },
    }
}

fn reassessment_window_metrics(
    label: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    activities: &[activities::Model],
    fitness_rows: &[fitness_freshness_daily::Model],
) -> ReassessmentWindowMetrics {
    let mut activity_count = 0;
    let mut total_distance_meters = 0.0;
    let mut total_elevation_gain_meters = 0.0;
    let mut benchmark_rides = Vec::new();

    for activity in activities
        .iter()
        .filter(|activity| (start_date..=end_date).contains(&activity.started_at.date_naive()))
    {
        activity_count += 1;
        total_distance_meters += activity.distance_meters.unwrap_or_default().max(0.0);
        total_elevation_gain_meters += activity.elevation_gain_meters.unwrap_or_default().max(0.0);

        let samples = activity_samples(activity);
        let elapsed_seconds = activity_elapsed_seconds(activity, &samples);
        let is_long_ride = elapsed_seconds >= REASSESSMENT_LONG_RIDE_MIN_SECONDS
            || activity
                .distance_meters
                .is_some_and(|distance| distance >= REASSESSMENT_LONG_RIDE_MIN_DISTANCE_METERS);
        if !is_long_ride {
            continue;
        }

        let analysis = compare_analysis_from_activity(activity);
        let elapsed_speed_mph = activity.distance_meters.and_then(|distance| {
            (distance > 0.0 && elapsed_seconds > 0)
                .then(|| round_metric((distance / f64::from(elapsed_seconds)) * 2.236_936))
        });
        benchmark_rides.push(ReassessmentBenchmarkRideResponse {
            activity_id: activity.id,
            title: activity.title.clone(),
            started_at: activity.started_at,
            distance_miles: activity
                .distance_meters
                .map(|distance| round_metric(distance / 1609.344)),
            elevation_gain_feet: activity
                .elevation_gain_meters
                .map(|gain| round_metric(gain * FEET_PER_METER)),
            elapsed_seconds,
            moving_seconds: activity.moving_time_seconds,
            elapsed_speed_mph,
            moving_speed_mph: analysis
                .moving_speed_mps
                .map(|speed| round_metric(speed * 2.236_936)),
            climbing_density_feet_per_hour: analysis
                .climbing_density_feet_per_hour
                .map(round_metric),
            aerobic_decoupling_percent: analysis.aerobic_decoupling_percent.map(round_metric),
            late_speed_change_percent: analysis.late_speed_change_percent.map(round_metric),
            fatigue_index: worst_fatigue_index(&hourly_durability(&samples)),
        });
    }

    benchmark_rides.sort_by(|a, b| {
        b.elapsed_seconds
            .cmp(&a.elapsed_seconds)
            .then_with(|| b.started_at.cmp(&a.started_at))
    });

    let endurance_ride = benchmark_rides
        .iter()
        .cloned()
        .max_by(|a, b| reassessment_endurance_score(a).total_cmp(&reassessment_endurance_score(b)));
    let pace_ride = benchmark_rides
        .iter()
        .filter(|ride| ride.elapsed_speed_mph.is_some())
        .cloned()
        .max_by(|a, b| {
            a.elapsed_speed_mph
                .unwrap_or_default()
                .total_cmp(&b.elapsed_speed_mph.unwrap_or_default())
        });
    let climbing_density_ride = benchmark_rides
        .iter()
        .filter(|ride| ride.climbing_density_feet_per_hour.is_some())
        .cloned()
        .max_by(|a, b| {
            a.climbing_density_feet_per_hour
                .unwrap_or_default()
                .total_cmp(&b.climbing_density_feet_per_hour.unwrap_or_default())
        });
    let fitness_values = fitness_rows
        .iter()
        .filter(|row| (start_date..=end_date).contains(&row.day))
        .map(|row| row.fitness)
        .collect::<Vec<_>>();
    let latest_fitness = fitness_rows
        .iter()
        .rev()
        .find(|row| (start_date..=end_date).contains(&row.day))
        .map(|row| row.fitness);

    ReassessmentWindowMetrics {
        response: ReassessmentWindowResponse {
            label: label.to_string(),
            start_date: start_date.format("%Y-%m-%d").to_string(),
            end_date: end_date.format("%Y-%m-%d").to_string(),
            activity_count,
            long_ride_count: benchmark_rides.len() as i32,
            total_distance_miles: round_metric(total_distance_meters / 1609.344),
            total_elevation_gain_feet: round_metric(total_elevation_gain_meters * FEET_PER_METER),
            best_long_ride_distance_miles: endurance_ride
                .as_ref()
                .and_then(|ride| ride.distance_miles),
            best_long_ride_duration_seconds: endurance_ride
                .as_ref()
                .map(|ride| ride.elapsed_seconds),
            best_long_ride_speed_mph: pace_ride.as_ref().and_then(|ride| ride.elapsed_speed_mph),
            best_long_ride_climbing_density_feet_per_hour: climbing_density_ride
                .as_ref()
                .and_then(|ride| ride.climbing_density_feet_per_hour),
            aggregate_long_ride_climbing_density_feet_per_hour:
                aggregate_climbing_density_feet_per_hour(&benchmark_rides),
            median_long_ride_decoupling_percent: median(
                benchmark_rides
                    .iter()
                    .filter_map(|ride| ride.aerobic_decoupling_percent)
                    .collect(),
            )
            .map(round_metric),
            median_long_ride_late_speed_change_percent: median(
                benchmark_rides
                    .iter()
                    .filter_map(|ride| ride.late_speed_change_percent)
                    .collect(),
            )
            .map(round_metric),
            median_long_ride_fatigue_index: median(
                benchmark_rides
                    .iter()
                    .filter_map(|ride| ride.fatigue_index)
                    .collect(),
            )
            .map(round_metric),
            average_fitness: mean(fitness_values).map(round_metric),
            latest_fitness: latest_fitness.map(round_metric),
        },
        benchmark_rides,
        endurance_ride,
        pace_ride,
    }
}

fn reassessment_endurance_score(ride: &ReassessmentBenchmarkRideResponse) -> f64 {
    let duration_hours = f64::from(ride.elapsed_seconds.max(0)) / 3600.0;
    let distance_score = ride.distance_miles.unwrap_or_default() / 10.0;
    duration_hours.max(distance_score)
}

fn aggregate_climbing_density_feet_per_hour(
    rides: &[ReassessmentBenchmarkRideResponse],
) -> Option<f64> {
    let mut elevation_feet = 0.0;
    let mut duration_seconds = 0;

    for ride in rides {
        let Some(ride_elevation_feet) = ride.elevation_gain_feet else {
            continue;
        };
        let ride_duration_seconds = ride.moving_seconds.unwrap_or(ride.elapsed_seconds);
        if ride_elevation_feet <= 0.0 || ride_duration_seconds <= 0 {
            continue;
        }

        elevation_feet += ride_elevation_feet;
        duration_seconds += ride_duration_seconds;
    }

    (elevation_feet > 0.0 && duration_seconds > 0)
        .then(|| round_metric(elevation_feet / (f64::from(duration_seconds) / 3600.0)))
}

fn latest_benchmark_value<F>(
    metrics: Option<&ReassessmentWindowMetrics>,
    value: F,
) -> (Option<&ReassessmentBenchmarkRideResponse>, Option<f64>)
where
    F: Fn(&ReassessmentBenchmarkRideResponse) -> Option<f64>,
{
    let ride = metrics.and_then(|metrics| {
        metrics
            .benchmark_rides
            .iter()
            .filter(|ride| value(ride).is_some())
            .max_by(|a, b| a.started_at.cmp(&b.started_at))
    });
    (ride, ride.and_then(value))
}

fn latest_fitness_on_or_before(
    fitness_rows: &[fitness_freshness_daily::Model],
    day: NaiveDate,
) -> Option<f64> {
    fitness_rows
        .iter()
        .filter(|row| row.day <= day && row.fitness > 0.0)
        .max_by(|a, b| a.day.cmp(&b.day))
        .map(|row| row.fitness)
}

fn projected_stale_signal_value(
    last_known_value: Option<f64>,
    last_known_source: Option<&ReassessmentBenchmarkRideResponse>,
    fitness_rows: &[fitness_freshness_daily::Model],
    end_date: NaiveDate,
) -> (Option<f64>, Option<String>) {
    let (Some(value), Some(source)) = (last_known_value, last_known_source) else {
        return (None, None);
    };
    let source_day = source.started_at.date_naive();
    let Some(source_fitness) = latest_fitness_on_or_before(fitness_rows, source_day) else {
        return (None, None);
    };
    let Some(current_fitness) = latest_fitness_on_or_before(fitness_rows, end_date) else {
        return (None, None);
    };
    if source_fitness <= 0.0 || current_fitness <= 0.0 {
        return (None, None);
    }

    let fitness_ratio = (current_fitness / source_fitness).clamp(0.60, 1.05);
    let projected_value = round_metric(value * fitness_ratio);

    (
        Some(projected_value),
        Some(format!(
            "Fitness-adjusted from last known benchmark using CTL {} -> {}; not used for verdict.",
            round_metric(source_fitness),
            round_metric(current_fitness)
        )),
    )
}

fn reassessment_improvement(
    recent: &ReassessmentWindowResponse,
    spring: &ReassessmentWindowResponse,
) -> ReassessmentImprovementResponse {
    ReassessmentImprovementResponse {
        fitness_change: delta(recent.average_fitness, spring.average_fitness),
        fitness_change_percent: percent_delta(recent.average_fitness, spring.average_fitness),
        long_ride_speed_change_mph: delta(
            recent.best_long_ride_speed_mph,
            spring.best_long_ride_speed_mph,
        ),
        long_ride_speed_change_percent: percent_delta(
            recent.best_long_ride_speed_mph,
            spring.best_long_ride_speed_mph,
        ),
        long_ride_distance_change_miles: delta(
            recent.best_long_ride_distance_miles,
            spring.best_long_ride_distance_miles,
        ),
        long_ride_distance_change_percent: percent_delta(
            recent.best_long_ride_distance_miles,
            spring.best_long_ride_distance_miles,
        ),
        climbing_density_change_feet_per_hour: delta(
            recent.aggregate_long_ride_climbing_density_feet_per_hour,
            spring.aggregate_long_ride_climbing_density_feet_per_hour,
        ),
        climbing_density_change_percent: percent_delta(
            recent.aggregate_long_ride_climbing_density_feet_per_hour,
            spring.aggregate_long_ride_climbing_density_feet_per_hour,
        ),
    }
}

fn reassessment_ability_estimate(
    recent: &ReassessmentWindowMetrics,
    target: &ReassessmentTarget,
) -> ReassessmentAbilityEstimateResponse {
    let target_distance_miles = target
        .response
        .target_distance_meters
        .map(|meters| meters / 1609.344);
    let target_elevation_feet = target
        .response
        .target_elevation_gain_meters
        .map(|meters| meters * FEET_PER_METER);
    let current_speed = recent.response.best_long_ride_speed_mph;
    let current_climb_density = recent
        .response
        .aggregate_long_ride_climbing_density_feet_per_hour;
    let pace_limited_finish_seconds = match (target_distance_miles, current_speed) {
        (Some(distance), Some(speed)) if distance > 0.0 && speed > 0.0 => {
            Some((distance / speed * 3600.0).round() as i32)
        }
        _ => None,
    };
    let climbing_limited_finish_seconds = match (target_elevation_feet, current_climb_density) {
        (Some(elevation), Some(density)) if elevation > 0.0 && density > 0.0 => {
            Some((elevation / density * 3600.0).round() as i32)
        }
        _ => None,
    };
    let estimated_finish_seconds =
        match (pace_limited_finish_seconds, climbing_limited_finish_seconds) {
            (Some(pace), Some(climbing)) => Some(pace.max(climbing)),
            (Some(pace), None) => Some(pace),
            (None, Some(climbing)) => Some(climbing),
            (None, None) => None,
        };
    let limiter = match (pace_limited_finish_seconds, climbing_limited_finish_seconds) {
        (Some(pace), Some(climbing)) if climbing > pace => Some("climbing".to_string()),
        (Some(_), Some(_)) => Some("pace".to_string()),
        (Some(_), None) => Some("pace".to_string()),
        (None, Some(_)) => Some("climbing".to_string()),
        (None, None) => None,
    };
    let estimated_speed_mph = match (target_distance_miles, estimated_finish_seconds) {
        (Some(distance), Some(seconds)) if distance > 0.0 && seconds > 0 => {
            Some(round_metric(distance / (f64::from(seconds) / 3600.0)))
        }
        _ => None,
    };
    let detail = if estimated_finish_seconds.is_some() {
        "Estimated from current benchmark long rides against the saved course distance and elevation. This is a current-ability estimate, not a finish target."
    } else if target.response.target_distance_meters.is_none()
        || target.response.target_elevation_gain_meters.is_none()
    {
        "Save course distance and elevation before Bike can estimate current course ability."
    } else {
        "Bike needs at least one current benchmark long ride with pace or climbing-density data before it can estimate current course ability."
    };

    ReassessmentAbilityEstimateResponse {
        estimated_finish_seconds,
        estimated_speed_mph,
        pace_limited_finish_seconds,
        climbing_limited_finish_seconds,
        current_long_ride_speed_mph: current_speed,
        current_climb_density_feet_per_hour: current_climb_density,
        limiter,
        detail: detail.to_string(),
    }
}

fn reassessment_endurance_signal(
    recent: &ReassessmentWindowMetrics,
    spring: &ReassessmentWindowMetrics,
    prior_training: Option<&ReassessmentWindowMetrics>,
    target: &ReassessmentTarget,
    fitness_rows: &[fitness_freshness_daily::Model],
    range_end: NaiveDate,
) -> ReassessmentSignalResponse {
    let duration_hours = recent
        .response
        .best_long_ride_duration_seconds
        .map(|seconds| f64::from(seconds) / 3600.0);
    let baseline_hours = spring
        .response
        .best_long_ride_duration_seconds
        .map(|seconds| f64::from(seconds) / 3600.0);
    let distance = recent.response.best_long_ride_distance_miles;
    let fatigue = recent.response.median_long_ride_fatigue_index;
    let target_hours = target
        .response
        .target_finish_seconds
        .map(|seconds| f64::from(seconds) / 3600.0 * REASSESSMENT_LONG_RIDE_TARGET_RATIO);
    let target_miles = target
        .response
        .target_distance_meters
        .map(|meters| (meters / 1609.344) * REASSESSMENT_LONG_RIDE_TARGET_RATIO);
    let improved_duration =
        percent_delta(duration_hours, baseline_hours).unwrap_or_default() >= 10.0;
    let status = if recent.response.long_ride_count == 0 || spring.response.long_ride_count == 0 {
        ReassessmentVerdict::MissingData
    } else if target_hours
        .is_some_and(|hours_target| duration_hours.is_some_and(|hours| hours >= hours_target))
        || target_miles
            .is_some_and(|miles_target| distance.is_some_and(|miles| miles >= miles_target))
    {
        if fatigue.is_none_or(|value| value <= 25.0) {
            ReassessmentVerdict::OnTrack
        } else {
            ReassessmentVerdict::PlausibleButRisky
        }
    } else if target_hours.is_some_and(|hours_target| {
        duration_hours.is_some_and(|hours| hours >= hours_target * 0.70)
    }) || target_miles
        .is_some_and(|miles_target| distance.is_some_and(|miles| miles >= miles_target * 0.70))
        || improved_duration
    {
        ReassessmentVerdict::PlausibleButRisky
    } else {
        ReassessmentVerdict::NeedsMoreEvidence
    };
    let (last_known_source, last_known_value) = if duration_hours.is_none() {
        latest_benchmark_value(prior_training, |ride| {
            Some(f64::from(ride.elapsed_seconds) / 3600.0)
        })
    } else {
        (None, None)
    };
    let (projected_current_value, projection_detail) =
        projected_stale_signal_value(last_known_value, last_known_source, fitness_rows, range_end);

    reassessment_signal(
        status,
        "Endurance progression",
        match status {
            ReassessmentVerdict::OnTrack => "Current long-ride duration or distance is in the range that can support the active XC goal if terrain execution holds.",
            ReassessmentVerdict::PlausibleButRisky => "Current endurance is substantial or improving, but the best long ride is still short of a strong target-specific proof point.",
            ReassessmentVerdict::NeedsMoreEvidence => "Current long rides do not yet show enough duration or distance progression for this reassessment.",
            ReassessmentVerdict::MissingData => "No current long rides were available for endurance progression.",
        },
        duration_hours.map(round_metric),
        baseline_hours.map(round_metric),
        target_hours.map(round_metric),
        "h",
        recent.endurance_ride.as_ref(),
        spring.endurance_ride.as_ref(),
        last_known_value,
        last_known_source,
        projected_current_value,
        projection_detail,
        range_end,
    )
}

fn reassessment_climbing_density_signal(
    recent: &ReassessmentWindowMetrics,
    spring: &ReassessmentWindowMetrics,
    prior_training: Option<&ReassessmentWindowMetrics>,
    target: &ReassessmentTarget,
    fitness_rows: &[fitness_freshness_daily::Model],
    range_end: NaiveDate,
) -> ReassessmentSignalResponse {
    let current = recent
        .response
        .aggregate_long_ride_climbing_density_feet_per_hour;
    let baseline = spring
        .response
        .aggregate_long_ride_climbing_density_feet_per_hour;
    let target_value = target.response.target_climb_density_feet_per_hour;
    let improved = percent_delta(current, baseline).unwrap_or_default() >= 15.0;
    let status = if current.is_none() || baseline.is_none() {
        ReassessmentVerdict::MissingData
    } else if target_value.is_none() {
        if improved {
            ReassessmentVerdict::PlausibleButRisky
        } else {
            ReassessmentVerdict::NeedsMoreEvidence
        }
    } else if current.is_some_and(|value| value >= target_value.unwrap_or_default() * 0.90) {
        ReassessmentVerdict::OnTrack
    } else if current.is_some_and(|value| value >= target_value.unwrap_or_default() * 0.75)
        || improved
    {
        ReassessmentVerdict::PlausibleButRisky
    } else {
        ReassessmentVerdict::NeedsMoreEvidence
    };
    let detail = match status {
        ReassessmentVerdict::OnTrack => {
            "Training-block benchmark rides are close to the target event's required vertical-per-hour load in aggregate."
        }
        ReassessmentVerdict::PlausibleButRisky => {
            if target_value.is_none() {
                "Training-block climbing density is improving versus spring, but a finish target is still needed for required course density."
            } else {
                "Aggregate climbing density is improving or partially specific, but still leaves execution risk for the target finish time."
            }
        }
        ReassessmentVerdict::NeedsMoreEvidence => {
            if target_value.is_none() {
                "Training-block climbing density is below the spring benchmark. A finish target is still needed for required course density."
            } else {
                "Training-block long rides are not dense enough in aggregate to model the required climbing load."
            }
        }
        ReassessmentVerdict::MissingData if target_value.is_none() => {
            if target.response.target_finish_seconds.is_none() {
                "The saved XC goal needs a target finish time to calculate required climbing density. Current and spring ride density are shown as context."
            } else {
                "The active XC goal needs distance, elevation, and finish-time targets before required climbing density can be calculated."
            }
        }
        ReassessmentVerdict::MissingData if current.is_none() => {
            "No training-block long rides had enough elevation and time data for aggregate climbing density."
        }
        ReassessmentVerdict::MissingData if baseline.is_none() => {
            "No spring-baseline long ride had enough distance, elevation, and time data for climbing density."
        }
        ReassessmentVerdict::MissingData => {
            "Climbing-density data is incomplete for this reassessment."
        }
    };
    let (last_known_source, last_known_value) = if current.is_none() {
        latest_benchmark_value(prior_training, |ride| ride.climbing_density_feet_per_hour)
    } else {
        (None, None)
    };
    let (projected_current_value, projection_detail) =
        projected_stale_signal_value(last_known_value, last_known_source, fitness_rows, range_end);

    reassessment_signal(
        status,
        "Climbing density",
        detail,
        current,
        baseline,
        target_value,
        "ft/h",
        None,
        None,
        last_known_value,
        last_known_source,
        projected_current_value,
        projection_detail,
        range_end,
    )
}

fn reassessment_long_ride_pace_signal(
    recent: &ReassessmentWindowMetrics,
    spring: &ReassessmentWindowMetrics,
    prior_training: Option<&ReassessmentWindowMetrics>,
    target: &ReassessmentTarget,
    fitness_rows: &[fitness_freshness_daily::Model],
    range_end: NaiveDate,
) -> ReassessmentSignalResponse {
    let current = recent.response.best_long_ride_speed_mph;
    let baseline = spring.response.best_long_ride_speed_mph;
    let target_value = target.response.target_speed_mph;
    let improved = percent_delta(current, baseline).unwrap_or_default() >= 5.0;
    let status = if current.is_none() || baseline.is_none() {
        ReassessmentVerdict::MissingData
    } else if target_value.is_none() {
        if improved {
            ReassessmentVerdict::PlausibleButRisky
        } else {
            ReassessmentVerdict::NeedsMoreEvidence
        }
    } else if current.is_some_and(|value| value >= target_value.unwrap_or_default() * 1.05) {
        ReassessmentVerdict::OnTrack
    } else if current.is_some_and(|value| value >= target_value.unwrap_or_default() * 0.95)
        || improved
    {
        ReassessmentVerdict::PlausibleButRisky
    } else {
        ReassessmentVerdict::NeedsMoreEvidence
    };
    let detail = match status {
        ReassessmentVerdict::OnTrack => {
            "Current long-ride elapsed pace clears the arithmetic target pace with some route-sensitivity buffer."
        }
        ReassessmentVerdict::PlausibleButRisky => {
            if target_value.is_none() {
                "Current long-ride elapsed pace is improving versus spring, but a finish target is still needed for target pace."
            } else {
                "Current long-ride pace is near target or improving, but the margin is thin for technical singletrack."
            }
        }
        ReassessmentVerdict::NeedsMoreEvidence => {
            if target_value.is_none() {
                "Current long-ride elapsed pace is below the spring benchmark. A finish target is still needed for target pace."
            } else {
                "Current long-ride pace is not yet close enough to the active goal's arithmetic requirement."
            }
        }
        ReassessmentVerdict::MissingData if target_value.is_none() => {
            if target.response.target_finish_seconds.is_none() {
                "The saved XC goal needs a target finish time to calculate required elapsed pace. Current and spring elapsed pace are shown as context."
            } else {
                "The active XC goal needs distance and finish-time targets before required elapsed pace can be calculated."
            }
        }
        ReassessmentVerdict::MissingData if current.is_none() => {
            "No current long ride had enough distance and elapsed-time data for pace."
        }
        ReassessmentVerdict::MissingData if baseline.is_none() => {
            "No spring-baseline long ride had enough distance and elapsed-time data for pace."
        }
        ReassessmentVerdict::MissingData => "Long-ride pace data is incomplete for this reassessment.",
    };
    let (last_known_source, last_known_value) = if current.is_none() {
        latest_benchmark_value(prior_training, |ride| ride.elapsed_speed_mph)
    } else {
        (None, None)
    };
    let (projected_current_value, projection_detail) =
        projected_stale_signal_value(last_known_value, last_known_source, fitness_rows, range_end);

    reassessment_signal(
        status,
        "Long-ride pace",
        detail,
        current,
        baseline,
        target_value,
        "mph",
        recent.pace_ride.as_ref(),
        spring.pace_ride.as_ref(),
        last_known_value,
        last_known_source,
        projected_current_value,
        projection_detail,
        range_end,
    )
}

fn reassessment_fitness_delta_signal(
    recent: &ReassessmentWindowResponse,
    spring: &ReassessmentWindowResponse,
    range_end: NaiveDate,
) -> ReassessmentSignalResponse {
    let current = recent.latest_fitness.or(recent.average_fitness);
    let baseline = spring.average_fitness.or(spring.latest_fitness);
    let change = delta(current, baseline).unwrap_or_default();
    let change_percent = percent_delta(current, baseline).unwrap_or_default();
    let status = if current.is_none() || baseline.is_none() {
        ReassessmentVerdict::MissingData
    } else if change >= 5.0 && change_percent >= 10.0 {
        ReassessmentVerdict::OnTrack
    } else if change >= 2.0 || change_percent >= 5.0 {
        ReassessmentVerdict::PlausibleButRisky
    } else {
        ReassessmentVerdict::NeedsMoreEvidence
    };

    reassessment_signal(
        status,
        "Fitness vs spring baseline",
        match status {
            ReassessmentVerdict::OnTrack => {
                "Current training-block fitness is materially higher than the spring baseline."
            }
            ReassessmentVerdict::PlausibleButRisky => {
                "Current training-block fitness is somewhat higher than spring, but the gain is modest."
            }
            ReassessmentVerdict::NeedsMoreEvidence => {
                "Current training-block fitness is not materially better than the spring baseline."
            }
            ReassessmentVerdict::MissingData => {
                "Fitness freshness rows were not available for both training-block and spring windows."
            }
        },
        current,
        baseline,
        None,
        "CTL",
        None,
        None,
        None,
        None,
        None,
        None,
        range_end,
    )
}

fn reassessment_signal(
    status: ReassessmentVerdict,
    title: &str,
    detail: &str,
    current_value: Option<f64>,
    baseline_value: Option<f64>,
    target_value: Option<f64>,
    unit: &str,
    current_source: Option<&ReassessmentBenchmarkRideResponse>,
    baseline_source: Option<&ReassessmentBenchmarkRideResponse>,
    last_known_value: Option<f64>,
    last_known_source: Option<&ReassessmentBenchmarkRideResponse>,
    projected_current_value: Option<f64>,
    projection_detail: Option<String>,
    range_end: NaiveDate,
) -> ReassessmentSignalResponse {
    ReassessmentSignalResponse {
        status,
        title: title.to_string(),
        detail: detail.to_string(),
        current_value: current_value.map(round_metric),
        projected_current_value: projected_current_value.map(round_metric),
        baseline_value: baseline_value.map(round_metric),
        target_value: target_value.map(round_metric),
        unit: unit.to_string(),
        current_source_activity_id: current_source.map(|ride| ride.activity_id),
        current_source_title: current_source.map(|ride| ride.title.clone()),
        current_source_started_at: current_source.map(|ride| ride.started_at),
        last_known_value: last_known_value.map(round_metric),
        last_known_source_activity_id: last_known_source.map(|ride| ride.activity_id),
        last_known_source_title: last_known_source.map(|ride| ride.title.clone()),
        last_known_source_started_at: last_known_source.map(|ride| ride.started_at),
        last_known_days_old: last_known_source
            .map(|ride| (range_end - ride.started_at.date_naive()).num_days().max(0)),
        projection_detail,
        baseline_source_activity_id: baseline_source.map(|ride| ride.activity_id),
        baseline_source_title: baseline_source.map(|ride| ride.title.clone()),
        baseline_source_started_at: baseline_source.map(|ride| ride.started_at),
    }
}

fn reassessment_overall_verdict(signals: &[ReassessmentVerdict]) -> ReassessmentVerdict {
    let missing = signals
        .iter()
        .filter(|status| **status == ReassessmentVerdict::MissingData)
        .count();
    if missing > 0 {
        return ReassessmentVerdict::MissingData;
    }

    let on_track = signals
        .iter()
        .filter(|status| **status == ReassessmentVerdict::OnTrack)
        .count();
    let plausible = signals
        .iter()
        .filter(|status| **status == ReassessmentVerdict::PlausibleButRisky)
        .count();
    let unsupported = signals
        .iter()
        .filter(|status| **status == ReassessmentVerdict::NeedsMoreEvidence)
        .count();

    if on_track >= 3 && unsupported == 0 {
        ReassessmentVerdict::OnTrack
    } else if on_track + plausible >= 2 && unsupported <= 1 {
        ReassessmentVerdict::PlausibleButRisky
    } else {
        ReassessmentVerdict::NeedsMoreEvidence
    }
}

fn reassessment_verdict_copy(
    verdict: ReassessmentVerdict,
    target: &ReassessmentTargetResponse,
    recent: &ReassessmentWindowResponse,
    spring: &ReassessmentWindowResponse,
    fitness_delta: &ReassessmentSignalResponse,
    ability_estimate: &ReassessmentAbilityEstimateResponse,
) -> (String, String) {
    if target.target_finish_seconds.is_none()
        && target.target_source == ReassessmentTargetSource::SavedGoal
        && recent.long_ride_count > 0
        && spring.long_ride_count > 0
        && fitness_delta.status != ReassessmentVerdict::MissingData
    {
        return match verdict {
            ReassessmentVerdict::OnTrack | ReassessmentVerdict::PlausibleButRisky => (
                "Current ability estimate available".to_string(),
                if ability_estimate.estimated_finish_seconds.is_some() {
                    "Bike can estimate current course ability from current benchmark rides, but a target finish time is still needed to decide whether the desired result is supported.".to_string()
                } else {
                    "Bike can compare current and spring benchmark evidence, but needs a target finish time to decide whether the desired result is supported.".to_string()
                },
            ),
            ReassessmentVerdict::NeedsMoreEvidence => (
                "Current ability needs more evidence".to_string(),
                if ability_estimate.estimated_finish_seconds.is_some() {
                    "Bike can estimate current course ability from benchmark rides. Endurance, pace, climbing density, or fitness still needs more proof before race-specific targeting.".to_string()
                } else {
                    "Bike has training-block and spring benchmark rides, but the current evidence needs more proof before race-specific targeting.".to_string()
                },
            ),
            ReassessmentVerdict::MissingData => (
                "Not enough data to estimate ability".to_string(),
                "Bike found a saved course goal, but still needs enough current rides, spring baseline rides, and fitness rows to estimate current ability.".to_string(),
            ),
        };
    }

    match verdict {
        ReassessmentVerdict::OnTrack => (
            "Goal is supported by current evidence".to_string(),
            "The current window shows enough specificity across endurance, climbing load, pace, and fitness delta to keep the active XC goal live.".to_string(),
        ),
        ReassessmentVerdict::PlausibleButRisky => (
            "Goal is plausible, but still high risk".to_string(),
            "Current evidence is better than baseline in meaningful places, but one or more target-specific demands still needs proof.".to_string(),
        ),
        ReassessmentVerdict::NeedsMoreEvidence => (
            "Goal needs more evidence".to_string(),
            "The training-block data still needs more long-ride durability, climbing density, pace, or fitness proof before the target is strongly supported.".to_string(),
        ),
        ReassessmentVerdict::MissingData => {
            if target.target_source == ReassessmentTargetSource::MissingGoal {
                (
                    "XC goal required".to_string(),
                    "The report found training evidence, but the active XC goal is missing distance or elevation, so target-specific pace and climbing-density checks cannot be calculated.".to_string(),
                )
            } else if recent.long_ride_count == 0 {
                (
                    "Current long ride required".to_string(),
                    "The report needs at least one current benchmark long ride to reassess the active XC goal.".to_string(),
                )
            } else if spring.long_ride_count == 0 {
                (
                    "Spring baseline required".to_string(),
                    "The report needs spring-baseline benchmark rides to tell whether recent long-ride evidence is materially better.".to_string(),
                )
            } else if fitness_delta.status == ReassessmentVerdict::MissingData {
                (
                    "Fitness baseline required".to_string(),
                    "The report has ride evidence, but fitness/freshness rows are missing for one of the comparison windows.".to_string(),
                )
            } else {
                (
                    "Not enough data to reassess".to_string(),
                    "One or more required reassessment signals is missing enough data to make a defensible call.".to_string(),
                )
            }
        }
    }
}

fn delta(current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    Some(round_metric(current? - baseline?))
}

fn percent_delta(current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    let current = current?;
    let baseline = baseline?;
    (baseline.abs() > f64::EPSILON)
        .then_some(round_metric(((current - baseline) / baseline) * 100.0))
}

#[derive(Debug, Clone)]
struct CompareRideAnalysis {
    activity_id: i32,
    distance_meters: Option<f64>,
    elevation_gain_meters: Option<f64>,
    elapsed_seconds: i32,
    moving_seconds: Option<i32>,
    moving_speed_mps: Option<f64>,
    aerobic_decoupling_percent: Option<f64>,
    median_climb_rate: Option<f64>,
    median_60s_hr_recovery_bpm: Option<f64>,
    late_speed_change_percent: Option<f64>,
    stopped_time_percent: Option<f64>,
    climbing_density_feet_per_hour: Option<f64>,
    z2_average_speed_mps: Option<f64>,
    z2_seconds: i32,
}

fn compare_analysis_from_activity(activity: &activities::Model) -> CompareRideAnalysis {
    let samples = activity_samples(activity);
    let elapsed_seconds = activity_elapsed_seconds(activity, &samples);
    let moving_seconds = activity.moving_time_seconds;
    let moving_speed_mps = activity
        .distance_meters
        .zip(moving_seconds)
        .and_then(|(distance, seconds)| {
            (distance > 0.0 && seconds > 0).then_some(distance / f64::from(seconds))
        })
        .or(activity.average_speed_mps);
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
    let median_60s_hr_recovery_bpm = median(
        climbs
            .iter()
            .filter_map(|climb| climb.heart_rate_recovery_60_seconds_bpm.map(f64::from))
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
    let z2_average_speed_mps = z2_average_speed(&samples, activity);

    CompareRideAnalysis {
        activity_id: activity.id,
        distance_meters: activity.distance_meters,
        elevation_gain_meters: activity.elevation_gain_meters,
        elapsed_seconds,
        moving_seconds,
        moving_speed_mps,
        aerobic_decoupling_percent,
        median_climb_rate,
        median_60s_hr_recovery_bpm,
        late_speed_change_percent,
        stopped_time_percent,
        climbing_density_feet_per_hour,
        z2_average_speed_mps,
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
        trend: compare_metric_trend(direction, unit, analyses, &value_for),
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

fn compare_metric_trend<F>(
    direction: &str,
    unit: Option<&str>,
    analyses: &[CompareRideAnalysis],
    value_for: &F,
) -> Option<CompareRideMetricTrendResponse>
where
    F: Fn(&CompareRideAnalysis) -> Option<f64>,
{
    if analyses.len() < 2 {
        return None;
    }

    let first = analyses.first()?;
    let latest = analyses.last()?;
    let first_value = value_for(first)?;
    let latest_value = value_for(latest)?;
    let change = latest_value - first_value;
    let change_percent =
        (first_value.abs() > f64::EPSILON).then_some((change / first_value) * 100.0);
    let interpretation = match direction {
        "higher" if change > 0.0 => "improving",
        "higher" if change < 0.0 => "declining",
        "lower" if change < 0.0 => "improving",
        "lower" if change > 0.0 => "declining",
        "neutral" => "route_sensitive",
        _ => "flat",
    };

    Some(CompareRideMetricTrendResponse {
        first_activity_id: first.activity_id,
        latest_activity_id: latest.activity_id,
        change: Some(round_metric(change)),
        change_percent: change_percent.map(round_metric),
        display: display_metric_change(change, change_percent, unit),
        interpretation: interpretation.to_string(),
    })
}

fn display_metric_value(value: Option<f64>, unit: Option<&str>) -> String {
    match (value, unit) {
        (Some(value), Some("h")) => format!("{value:.1}h"),
        (Some(value), Some("%")) => format!("{value:.1}%"),
        (Some(value), Some("mph")) => format!("{value:.1} mph"),
        (Some(value), Some("bpm")) => format!("{value:.0} bpm"),
        (Some(value), Some(unit)) => format!("{value:.0} {unit}"),
        (Some(value), None) => format!("{value:.1}"),
        (None, _) => "n/a".to_string(),
    }
}

fn display_metric_change(change: f64, change_percent: Option<f64>, unit: Option<&str>) -> String {
    let sign = if change > 0.0 { "+" } else { "" };
    let value = match unit {
        Some("h") => format!("{sign}{change:.1}h"),
        Some("%") => format!("{sign}{change:.1}%"),
        Some("mph") => format!("{sign}{change:.1} mph"),
        Some("bpm") => format!("{sign}{change:.0} bpm"),
        Some(unit) => format!("{sign}{change:.0} {unit}"),
        None => format!("{sign}{change:.1}"),
    };
    match change_percent {
        Some(percent) => {
            let percent_sign = if percent > 0.0 { "+" } else { "" };
            format!("{value} ({percent_sign}{percent:.1}%)")
        }
        None => value,
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
        let duration_seconds = (end - start).max(1);
        let duration_hours = f64::from(duration_seconds) / 3600.0;
        let climb_rate_meters_per_hour = (ascent > 0.0 && moving_seconds > 0)
            .then_some(ascent / (f64::from(moving_seconds) / 3600.0));

        rows.push(HourlyDurabilityResponse {
            hour,
            elapsed_start_seconds: start,
            elapsed_end_seconds: end,
            distance_meters: distance.map(round_metric),
            average_speed_mps: average_speed.map(round_metric),
            average_heart_rate_bpm: average_hr.map(round_metric),
            max_heart_rate_bpm: max_hr,
            ascent_meters: round_metric(ascent),
            climb_rate_meters_per_hour: climb_rate_meters_per_hour.map(round_metric),
            moving_seconds,
            stopped_seconds,
            stop_count,
            stop_frequency_per_hour: round_metric(f64::from(stop_count) / duration_hours),
            efficiency_mps_per_bpm: efficiency(&segment).map(round_metric),
            fatigue_index: None,
        });
    }

    apply_fatigue_indexes(&mut rows);
    rows
}

fn fatigue_start_hour(hourly: &[HourlyDurabilityResponse]) -> Option<i32> {
    hourly
        .iter()
        .find(|row| {
            row.hour >= 2
                && row
                    .fatigue_index
                    .is_some_and(|fatigue_index| fatigue_index >= 25.0)
        })
        .map(|row| row.hour)
}

fn worst_fatigue_index(hourly: &[HourlyDurabilityResponse]) -> Option<f64> {
    hourly
        .iter()
        .filter_map(|row| row.fatigue_index)
        .max_by(f64::total_cmp)
        .map(round_metric)
}

fn apply_fatigue_indexes(rows: &mut [HourlyDurabilityResponse]) {
    let baseline = FatigueBaseline {
        efficiency: median(
            rows.iter()
                .take(2)
                .filter_map(|row| row.efficiency_mps_per_bpm)
                .collect(),
        ),
        speed: median(
            rows.iter()
                .take(2)
                .filter_map(|row| row.average_speed_mps)
                .collect(),
        ),
        climb_rate: median(
            rows.iter()
                .take(2)
                .filter_map(|row| row.climb_rate_meters_per_hour)
                .collect(),
        ),
        stop_frequency: median(
            rows.iter()
                .take(2)
                .map(|row| row.stop_frequency_per_hour)
                .collect(),
        ),
        heart_rate: median(
            rows.iter()
                .take(2)
                .filter_map(|row| row.average_heart_rate_bpm)
                .collect(),
        ),
    };

    for row in rows {
        row.fatigue_index = fatigue_index_for_row(row, &baseline).map(round_metric);
    }
}

#[derive(Debug, Clone, Copy)]
struct FatigueBaseline {
    efficiency: Option<f64>,
    speed: Option<f64>,
    climb_rate: Option<f64>,
    stop_frequency: Option<f64>,
    heart_rate: Option<f64>,
}

fn fatigue_index_for_row(
    row: &HourlyDurabilityResponse,
    baseline: &FatigueBaseline,
) -> Option<f64> {
    let mut score = 0.0;
    let mut evidence_count = 0;

    if let (Some(baseline_efficiency), Some(efficiency)) =
        (baseline.efficiency, row.efficiency_mps_per_bpm)
    {
        if baseline_efficiency > f64::EPSILON {
            let drop = ((baseline_efficiency - efficiency) / baseline_efficiency).max(0.0);
            score += (drop * 180.0).min(45.0);
            evidence_count += 1;
        }
    }

    if let (Some(baseline_speed), Some(speed)) = (baseline.speed, row.average_speed_mps) {
        if baseline_speed > f64::EPSILON {
            let drop = ((baseline_speed - speed) / baseline_speed).max(0.0);
            let heart_rate_pressure = match (baseline.heart_rate, row.average_heart_rate_bpm) {
                (Some(baseline_hr), Some(hr)) if hr >= baseline_hr - 3.0 => 1.15,
                _ => 1.0,
            };
            score += ((drop * 100.0) * heart_rate_pressure).min(25.0);
            evidence_count += 1;
        }
    }

    if let (Some(baseline_climb_rate), Some(climb_rate)) =
        (baseline.climb_rate, row.climb_rate_meters_per_hour)
    {
        if baseline_climb_rate > f64::EPSILON {
            let drop = ((baseline_climb_rate - climb_rate) / baseline_climb_rate).max(0.0);
            score += (drop * 100.0).min(20.0);
            evidence_count += 1;
        }
    }

    if let Some(baseline_stop_frequency) = baseline.stop_frequency {
        let added_stops = (row.stop_frequency_per_hour - baseline_stop_frequency).max(0.0);
        score += (added_stops * 4.0).min(10.0);
        evidence_count += 1;
    }

    (evidence_count >= 2).then_some(score.clamp(0.0, 100.0))
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

fn z2_average_speed(samples: &[TrendSample], activity: &activities::Model) -> Option<f64> {
    let zone = deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref())
        .into_iter()
        .find(|zone| zone.zone == 2)?;
    let min_bpm = zone.min_bpm?;
    let max_bpm = zone.max_bpm?;
    let mut z2_distance = 0.0;
    let mut z2_seconds = 0;

    for window in samples.windows(2) {
        let previous = window[0];
        let current = window[1];
        let delta_time = current.elapsed_seconds - previous.elapsed_seconds;
        if delta_time <= 0 {
            continue;
        }

        let Some(heart_rate_bpm) = previous.heart_rate_bpm.or(current.heart_rate_bpm) else {
            continue;
        };
        if heart_rate_bpm < min_bpm || heart_rate_bpm > max_bpm {
            continue;
        }

        let Some(delta_distance) = delta_distance(previous, current) else {
            continue;
        };

        z2_distance += delta_distance;
        z2_seconds += delta_time;
    }

    (z2_distance > 0.0 && z2_seconds > 0).then_some(z2_distance / f64::from(z2_seconds))
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
    use crate::activity_details::{serialize_derived_activity_data, ActivityDerivedData};
    use crate::training_profile::{
        serialize_activity_heart_rate_zones, ActivityHeartRateZoneSummary,
    };

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

    fn compare_test_activity(
        id: i32,
        title: &str,
        started_at: DateTime<Utc>,
        distance_meters: f64,
        moving_time_seconds: i32,
        z2_distance_meters: f64,
    ) -> activities::Model {
        let mut activity = test_activity();
        activity.id = id;
        activity.title = title.to_string();
        activity.started_at = started_at;
        activity.distance_meters = Some(distance_meters);
        activity.moving_time_seconds = Some(moving_time_seconds);
        activity.total_time_seconds = Some(moving_time_seconds);
        activity.average_speed_mps = Some(distance_meters / f64::from(moving_time_seconds));
        activity.heart_rate_zones_json = serialize_activity_heart_rate_zones(&[
            ActivityHeartRateZoneSummary {
                zone: 1,
                label: "Z1".to_string(),
                min_bpm: None,
                max_bpm: Some(120),
                duration_seconds: 0,
                share_percent: 0.0,
            },
            ActivityHeartRateZoneSummary {
                zone: 2,
                label: "Z2".to_string(),
                min_bpm: Some(121),
                max_bpm: Some(140),
                duration_seconds: moving_time_seconds,
                share_percent: 100.0,
            },
        ])
        .unwrap();
        activity.derived_data_json = Some(
            serialize_derived_activity_data(&ActivityDerivedData {
                chart_points: vec![
                    ActivityChartPoint {
                        elapsed_seconds: 0,
                        distance_meters: Some(0.0),
                        elevation_meters: Some(100.0),
                        speed_mps: None,
                        heart_rate_bpm: Some(130),
                        cadence_rpm: None,
                        power_watts: None,
                    },
                    ActivityChartPoint {
                        elapsed_seconds: moving_time_seconds,
                        distance_meters: Some(z2_distance_meters),
                        elevation_meters: Some(100.0),
                        speed_mps: None,
                        heart_rate_bpm: Some(130),
                        cadence_rpm: None,
                        power_watts: None,
                    },
                ],
                ..ActivityDerivedData::default()
            })
            .unwrap(),
        );
        activity
    }

    fn reassessment_test_activity(
        id: i32,
        title: &str,
        started_at: DateTime<Utc>,
        distance_miles: f64,
        elevation_feet: f64,
        moving_seconds: i32,
        elapsed_seconds: i32,
    ) -> activities::Model {
        let mut activity = test_activity();
        activity.id = id;
        activity.title = title.to_string();
        activity.started_at = started_at;
        activity.distance_meters = Some(distance_miles * 1609.344);
        activity.elevation_gain_meters = Some(elevation_feet / FEET_PER_METER);
        activity.moving_time_seconds = Some(moving_seconds);
        activity.total_time_seconds = Some(elapsed_seconds);
        activity.average_speed_mps = Some((distance_miles * 1609.344) / f64::from(moving_seconds));
        activity
    }

    fn test_preferences() -> user_preferences::Model {
        let now = Utc::now();
        user_preferences::Model {
            id: 1,
            user_id: 7,
            unit_system: "imperial".to_string(),
            estimated_ftp_watts: None,
            heart_rate_zone_bounds_json: None,
            xc_goal_start_date: Some(NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()),
            xc_goal_target_date: Some(NaiveDate::from_ymd_opt(2026, 9, 20).unwrap()),
            xc_goal_target_distance_meters: Some(160_934.4),
            xc_goal_target_elevation_gain_meters: Some(3_962.4),
            xc_goal_event_name: Some("Configured event".to_string()),
            xc_goal_target_finish_time_seconds: Some(43_200),
            xc_goal_event_profile: Some("technical_singletrack".to_string()),
            xc_goal_backfill_status: None,
            xc_goal_backfill_completed_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn fitness_row(id: i32, day: NaiveDate, fitness: f64) -> fitness_freshness_daily::Model {
        let now = Utc::now();
        fitness_freshness_daily::Model {
            id,
            user_id: 7,
            day,
            activity_count: 1,
            training_load: 0.0,
            fitness,
            fatigue: fitness,
            form: 0.0,
            created_at: now,
            updated_at: now,
        }
    }

    fn reassessment_test_activities() -> Vec<activities::Model> {
        vec![
            reassessment_test_activity(
                1,
                "Spring endurance",
                Utc.with_ymd_and_hms(2026, 4, 1, 12, 0, 0).single().unwrap(),
                45.0,
                1_000.0,
                14_400,
                14_400,
            ),
            reassessment_test_activity(
                2,
                "Spring pace",
                Utc.with_ymd_and_hms(2026, 4, 8, 12, 0, 0).single().unwrap(),
                36.0,
                500.0,
                10_800,
                10_800,
            ),
            reassessment_test_activity(
                3,
                "Spring climbing",
                Utc.with_ymd_and_hms(2026, 4, 15, 12, 0, 0)
                    .single()
                    .unwrap(),
                35.0,
                3_000.0,
                10_800,
                10_800,
            ),
            reassessment_test_activity(
                4,
                "Recent endurance",
                Utc.with_ymd_and_hms(2026, 8, 1, 12, 0, 0).single().unwrap(),
                60.0,
                2_000.0,
                28_800,
                28_800,
            ),
            reassessment_test_activity(
                5,
                "Recent pace",
                Utc.with_ymd_and_hms(2026, 8, 8, 12, 0, 0).single().unwrap(),
                45.0,
                1_000.0,
                10_800,
                14_400,
            ),
            reassessment_test_activity(
                6,
                "Recent climbing",
                Utc.with_ymd_and_hms(2026, 8, 15, 12, 0, 0)
                    .single()
                    .unwrap(),
                36.0,
                5_000.0,
                12_600,
                12_600,
            ),
        ]
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

    fn hourly_sample_series() -> Vec<TrendSample> {
        let mut samples = Vec::new();
        let mut distance_meters = 0.0;
        let mut elevation_meters = 100.0;

        for elapsed_seconds in (0..=10_800).step_by(30) {
            let (speed_mps, heart_rate_bpm, climb_rate_meters_per_hour) =
                if elapsed_seconds <= 7_200 {
                    (5.0, 130, 120.0)
                } else {
                    (3.0, 132, 40.0)
                };

            if elapsed_seconds > 0 {
                distance_meters += speed_mps * 30.0;
                elevation_meters += climb_rate_meters_per_hour * (30.0 / 3600.0);
            }

            samples.push(TrendSample {
                elapsed_seconds,
                distance_meters: Some(distance_meters),
                elevation_meters: Some(elevation_meters),
                speed_mps: Some(speed_mps),
                heart_rate_bpm: Some(heart_rate_bpm),
                cadence_rpm: Some(80),
                power_watts: None,
            });
        }

        samples
    }

    #[test]
    fn report_registry_declares_initial_reports_and_compare_filters() {
        let definitions = report_definitions();

        assert_eq!(definitions.len(), 7);
        assert!(definitions
            .iter()
            .any(|definition| definition.id == ReportId::AggregateTrends));

        let compare = definitions
            .iter()
            .find(|definition| definition.id == ReportId::CompareRides)
            .unwrap();
        assert!(compare
            .supported_filters
            .contains(&ReportFilterKey::ActivityIds));
        assert!(compare
            .supported_filters
            .contains(&ReportFilterKey::MinDuration));
        assert!(compare
            .supported_filters
            .contains(&ReportFilterKey::MinDistance));
        assert!(compare
            .metrics
            .iter()
            .any(|metric| metric.key == "aerobic_decoupling_percent"
                && metric.direction == ReportMetricDirection::Lower));

        let reassessment = definitions
            .iter()
            .find(|definition| definition.id == ReportId::Reassessment)
            .unwrap();
        assert!(reassessment
            .required_data_quality
            .contains(&"fitness_freshness".to_string()));
        assert!(reassessment
            .metrics
            .iter()
            .any(|metric| metric.key == "long_ride_pace"
                && metric.direction == ReportMetricDirection::Higher));
    }

    #[test]
    fn parses_report_ids_and_rejects_unknown_reports() {
        assert_eq!(parse_report_id(None).unwrap(), ReportId::AggregateTrends);
        assert_eq!(
            parse_report_id(Some("compare_rides")).unwrap(),
            ReportId::CompareRides
        );
        assert_eq!(
            parse_report_id(Some("reassessment")).unwrap(),
            ReportId::Reassessment
        );
        assert!(parse_report_id(Some("race_readiness")).is_err());
    }

    #[test]
    fn reassessment_target_uses_saved_goal_before_defaults() {
        let preferences = test_preferences();
        let target = reassessment_target(Some(&preferences)).response;

        assert_eq!(target.event_name, "Configured event");
        assert_eq!(target.target_source, ReassessmentTargetSource::SavedGoal);
        assert_eq!(target.target_finish_seconds, Some(43_200));
        assert_eq!(target.target_elevation_gain_meters, Some(3_962.4));
        assert_eq!(target.target_date.as_deref(), Some("2026-09-20"));

        let fallback = reassessment_target(None).response;
        assert_eq!(
            fallback.target_source,
            ReassessmentTargetSource::MissingGoal
        );
        assert_eq!(fallback.target_finish_seconds, None);
        assert_eq!(fallback.target_distance_meters, None);
    }

    #[test]
    fn reassessment_uses_independent_long_ride_evidence_sources() {
        let activities = reassessment_test_activities();
        let fitness_rows = vec![
            fitness_row(1, NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(), 50.0),
            fitness_row(2, NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(), 60.0),
        ];
        let preferences = test_preferences();

        let report = build_reassessment_report(
            &activities,
            &fitness_rows,
            Some(&preferences),
            preferences.xc_goal_start_date.unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
        );

        assert_eq!(report.recent_window.label, "Current training block");
        assert_eq!(report.recent_window.start_date, "2026-06-01");
        assert_eq!(report.recent_window.end_date, "2026-08-31");
        assert_eq!(report.verdict, ReassessmentVerdict::OnTrack);
        assert_eq!(
            report.endurance_progression.current_source_title.as_deref(),
            Some("Recent endurance")
        );
        assert_eq!(
            report.long_ride_pace.current_source_title.as_deref(),
            Some("Recent pace")
        );
        assert_eq!(report.long_ride_pace.current_value, Some(11.25));
        let pace_benchmark = report
            .benchmark_rides
            .iter()
            .find(|ride| ride.activity_id == 5)
            .unwrap();
        assert_eq!(pace_benchmark.elapsed_speed_mph, Some(11.25));
        assert_eq!(pace_benchmark.moving_speed_mph, Some(15.0));
        assert_eq!(
            report
                .recent_window
                .aggregate_long_ride_climbing_density_feet_per_hour,
            Some(551.72)
        );
        assert_eq!(
            report
                .spring_baseline_window
                .aggregate_long_ride_climbing_density_feet_per_hour,
            Some(450.0)
        );
        assert_eq!(report.climbing_density.current_value, Some(551.72));
        assert_eq!(report.climbing_density.baseline_value, Some(450.0));
        assert_eq!(report.climbing_density.current_source_title, None);
        assert_eq!(
            report.climbing_density.status,
            ReassessmentVerdict::PlausibleButRisky
        );
    }

    #[test]
    fn reassessment_current_window_does_not_reuse_spring_baseline_rides() {
        let activities = reassessment_test_activities();
        let fitness_rows = vec![
            fitness_row(1, NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(), 50.0),
            fitness_row(2, NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(), 60.0),
        ];
        let mut preferences = test_preferences();
        preferences.xc_goal_start_date = Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());

        let report = build_reassessment_report(
            &activities,
            &fitness_rows,
            Some(&preferences),
            preferences.xc_goal_start_date.unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
        );

        assert_eq!(report.recent_window.label, "Current training block");
        assert_eq!(report.recent_window.start_date, "2026-06-01");
        assert_eq!(report.spring_baseline_window.start_date, "2026-03-01");
        assert_eq!(report.spring_baseline_window.end_date, "2026-05-31");
        assert_eq!(
            report.long_ride_pace.current_source_title.as_deref(),
            Some("Recent pace")
        );
        assert_eq!(
            report.long_ride_pace.baseline_source_title.as_deref(),
            Some("Spring pace")
        );
        assert_eq!(report.long_ride_pace.current_source_activity_id, Some(5));
        assert_eq!(report.long_ride_pace.baseline_source_activity_id, Some(2));
        assert_ne!(
            report.long_ride_pace.current_source_activity_id,
            report.long_ride_pace.baseline_source_activity_id
        );
    }

    #[test]
    fn reassessment_stale_training_rides_are_last_known_not_current() {
        let activities = reassessment_test_activities()
            .into_iter()
            .filter(|activity| activity.id <= 3)
            .collect::<Vec<_>>();
        let fitness_rows = vec![
            fitness_row(1, NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(), 70.0),
            fitness_row(2, NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(), 65.0),
        ];
        let mut preferences = test_preferences();
        preferences.xc_goal_start_date = Some(NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());

        let report = build_reassessment_report(
            &activities,
            &fitness_rows,
            Some(&preferences),
            preferences.xc_goal_start_date.unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
        );

        assert_eq!(report.recent_window.start_date, "2026-06-01");
        assert_eq!(report.recent_window.long_ride_count, 0);
        assert_eq!(
            report.long_ride_pace.status,
            ReassessmentVerdict::MissingData
        );
        assert_eq!(report.long_ride_pace.current_value, None);
        assert_eq!(report.long_ride_pace.current_source_activity_id, None);
        assert_eq!(
            report.long_ride_pace.last_known_source_title.as_deref(),
            Some("Spring climbing")
        );
        assert_eq!(report.long_ride_pace.last_known_value, Some(11.67));
        assert_eq!(report.long_ride_pace.projected_current_value, Some(10.84));
        assert_eq!(report.long_ride_pace.last_known_days_old, Some(138));
        assert!(report
            .long_ride_pace
            .projection_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("not used for verdict")));
    }

    #[test]
    fn reassessment_missing_fitness_forces_missing_data_verdict() {
        let activities = reassessment_test_activities();
        let preferences = test_preferences();

        let report = build_reassessment_report(
            &activities,
            &[],
            Some(&preferences),
            preferences.xc_goal_start_date.unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
        );

        assert_eq!(
            report.fitness_delta.status,
            ReassessmentVerdict::MissingData
        );
        assert_eq!(report.verdict, ReassessmentVerdict::MissingData);
    }

    #[test]
    fn reassessment_missing_finish_time_still_estimates_current_ability() {
        let activities = reassessment_test_activities();
        let fitness_rows = vec![
            fitness_row(1, NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(), 50.0),
            fitness_row(2, NaiveDate::from_ymd_opt(2026, 8, 15).unwrap(), 60.0),
        ];
        let mut preferences = test_preferences();
        preferences.xc_goal_target_finish_time_seconds = None;

        let report = build_reassessment_report(
            &activities,
            &fitness_rows,
            Some(&preferences),
            preferences.xc_goal_start_date.unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
        );

        assert_ne!(report.verdict, ReassessmentVerdict::MissingData);
        assert_eq!(report.verdict_title, "Current ability estimate available");
        assert!(report
            .verdict_detail
            .contains("estimate current course ability"));
        assert!(report.ability_estimate.estimated_finish_seconds.is_some());
        assert_eq!(report.ability_estimate.limiter.as_deref(), Some("climbing"));
        assert_eq!(
            report.ability_estimate.current_long_ride_speed_mph,
            report.long_ride_pace.current_value
        );
        assert!(report.long_ride_pace.current_value.is_some());
        assert_ne!(
            report.long_ride_pace.status,
            ReassessmentVerdict::MissingData
        );
        assert!(report.long_ride_pace.detail.contains("finish target"));
        assert!(!report.long_ride_pace.detail.contains("No recent long ride"));
        assert!(report.climbing_density.current_value.is_some());
        assert_ne!(
            report.climbing_density.status,
            ReassessmentVerdict::MissingData
        );
        assert!(report.climbing_density.detail.contains("finish target"));
        assert!(!report
            .climbing_density
            .detail
            .contains("No recent long ride"));
    }

    #[test]
    fn reassessment_fitness_signal_uses_latest_training_block_fitness() {
        let activities = reassessment_test_activities();
        let fitness_rows = vec![
            fitness_row(1, NaiveDate::from_ymd_opt(2026, 4, 15).unwrap(), 50.0),
            fitness_row(2, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap(), 35.0),
            fitness_row(3, NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(), 60.0),
        ];
        let preferences = test_preferences();

        let report = build_reassessment_report(
            &activities,
            &fitness_rows,
            Some(&preferences),
            preferences.xc_goal_start_date.unwrap(),
            NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
        );

        assert_eq!(report.recent_window.average_fitness, Some(47.5));
        assert_eq!(report.recent_window.latest_fitness, Some(60.0));
        assert_eq!(report.fitness_delta.current_value, Some(60.0));
        assert_eq!(report.fitness_delta.status, ReassessmentVerdict::OnTrack);
    }

    #[test]
    fn reassessment_overall_verdict_covers_all_statuses() {
        assert_eq!(
            reassessment_overall_verdict(&[
                ReassessmentVerdict::OnTrack,
                ReassessmentVerdict::OnTrack,
                ReassessmentVerdict::PlausibleButRisky,
                ReassessmentVerdict::OnTrack,
            ]),
            ReassessmentVerdict::OnTrack
        );
        assert_eq!(
            reassessment_overall_verdict(&[
                ReassessmentVerdict::PlausibleButRisky,
                ReassessmentVerdict::PlausibleButRisky,
                ReassessmentVerdict::NeedsMoreEvidence,
                ReassessmentVerdict::OnTrack,
            ]),
            ReassessmentVerdict::PlausibleButRisky
        );
        assert_eq!(
            reassessment_overall_verdict(&[
                ReassessmentVerdict::NeedsMoreEvidence,
                ReassessmentVerdict::NeedsMoreEvidence,
                ReassessmentVerdict::PlausibleButRisky,
                ReassessmentVerdict::OnTrack,
            ]),
            ReassessmentVerdict::NeedsMoreEvidence
        );
        assert_eq!(
            reassessment_overall_verdict(&[
                ReassessmentVerdict::MissingData,
                ReassessmentVerdict::OnTrack,
                ReassessmentVerdict::OnTrack,
                ReassessmentVerdict::OnTrack,
            ]),
            ReassessmentVerdict::MissingData
        );
    }

    #[test]
    fn rejects_invalid_minimum_filters() {
        let query = TrainingReportsQuery {
            boundary: None,
            report: None,
            start_date: None,
            end_date: None,
            activity_ids: None,
            min_duration_seconds: Some(-1),
            min_distance_meters: None,
        };
        assert!(validate_report_filters(&query).is_err());

        let query = TrainingReportsQuery {
            min_duration_seconds: None,
            min_distance_meters: Some(f64::NAN),
            ..query
        };
        assert!(validate_report_filters(&query).is_err());
    }

    #[test]
    fn compare_rides_sorts_chronologically_and_reports_speed_trends() {
        let older = compare_test_activity(
            1,
            "Older benchmark",
            Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).single().unwrap(),
            16_093.44,
            3600,
            16_093.44,
        );
        let latest = compare_test_activity(
            2,
            "Latest benchmark",
            Utc.with_ymd_and_hms(2026, 2, 1, 12, 0, 0).single().unwrap(),
            19_312.128,
            3600,
            19_312.128,
        );

        let report = build_compare_rides_report(&[latest.clone(), older.clone()], &[latest, older]);

        assert_eq!(report.selected_rides[0].activity_id, 1);
        assert_eq!(report.selected_rides[1].activity_id, 2);

        let moving_speed = report
            .metrics
            .iter()
            .find(|metric| metric.key == "moving_speed_mph")
            .unwrap();
        assert_eq!(moving_speed.values[0].display, "10.0 mph");
        assert_eq!(moving_speed.values[1].display, "12.0 mph");
        assert_eq!(
            moving_speed.trend.as_ref().unwrap().display,
            "+2.0 mph (+20.0%)"
        );
        assert_eq!(
            moving_speed.trend.as_ref().unwrap().interpretation,
            "route_sensitive"
        );

        let z2_speed = report
            .metrics
            .iter()
            .find(|metric| metric.key == "z2_average_speed_mph")
            .unwrap();
        assert_eq!(z2_speed.values[0].display, "10.0 mph");
        assert_eq!(z2_speed.values[1].display, "12.0 mph");
        assert_eq!(z2_speed.trend.as_ref().unwrap().interpretation, "improving");
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

    #[test]
    fn hourly_durability_exposes_climb_rate_stop_frequency_and_fatigue_index() {
        let hourly = hourly_durability(&hourly_sample_series());

        assert_eq!(hourly.len(), 3);
        assert_eq!(hourly[0].stop_frequency_per_hour, 0.0);
        assert!(hourly[0].climb_rate_meters_per_hour.unwrap_or_default() > 100.0);
        assert!(hourly[2].climb_rate_meters_per_hour.unwrap_or_default() < 50.0);
        assert!(hourly[2].fatigue_index.unwrap_or_default() >= 25.0);
        assert_eq!(fatigue_start_hour(&hourly), Some(3));
    }
}
