use crate::activity_type::ActivityType;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::services::cooldown::{CooldownService, CooldownType};
use crate::services::reports::{PreparedTrainingReportRequest, ReportsService};
use crate::storage::AppStorage;
use axum::extract::{Query, State};
use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use kaleido::auth::UserContext;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReportId {
    RideSummary,
    Endurance,
    Climbing,
    Fatigue,
    CompareRides,
    Reassessment,
    Distance,
    Elevation,
    ActivityTypeTime,
    AggregateTrends,
}

impl ReportId {
    pub(crate) fn is_aggregate_bucket_report(self) -> bool {
        matches!(
            self,
            Self::Distance | Self::Elevation | Self::ActivityTypeTime | Self::AggregateTrends
        )
    }
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
    #[serde(rename = "3year")]
    ThreeYear,
    #[serde(rename = "5year")]
    FiveYear,
    All,
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
    pub distance_meters: f64,
    pub distance_miles: f64,
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
    pub activity_type_times: Vec<ActivityTypeTimeResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ActivityTypeTimeResponse {
    pub activity_type: ActivityType,
    pub seconds: i32,
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
    Json(ReportsService::definitions())
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
    let prepared_request =
        ReportsService::prepare_training_report_request(PreparedTrainingReportRequest {
            user_id: user.id,
            state: state.clone(),
            query,
            now: Utc::now(),
        })?;

    let mut cooldown =
        CooldownService::acquire(&state.db, CooldownType::ReportGeneration, user.id).await?;
    let response = ReportsService::build_training_report(prepared_request).await;
    if let Err(error) = cooldown.release().await {
        tracing::error!(error = ?error, "failed to release report generation cooldown");
        if response.is_ok() {
            return Err(error);
        }
    }

    response.map(Json)
}
