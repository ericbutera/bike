use crate::activity_training_analysis::ActivityRideFocus;
use crate::activity_type::ActivityType;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::services::training_goals::TrainingGoalsService;
use crate::storage::AppStorage;
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::UserContext;
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingGoalKey {
    WeeklyZ2Average,
    WeeklyClimbingAverage,
    AerobicDecoupling,
    DhLapsPerSession,
    DhRepeatFade,
    DhRollingTop3Gap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingMetricUnit {
    Seconds,
    Meters,
    Percent,
    Count,
    MetersPerSecond,
    MetersPerKilometer,
    MetersPerHour,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingGoalDirection {
    AtLeast,
    AtMost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRecommendationKey {
    BuildXcBaseline,
    RepeatComparableEnduranceRide,
    IncreaseEnduranceVolume,
    AddClimbingEndurance,
    HoldSteadyEndurance,
    MaintainEnduranceRhythm,
    RecoverBeforeNextXcRide,
    UsePositiveFormForXcBenchmark,
    MarkDhSegments,
    AddDhRepeats,
    ReduceDhFade,
    ChaseDhConsistency,
    MaintainDhMomentum,
    RecoverBeforeNextDhSession,
    UsePositiveFormForDhBenchmark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrainingRecommendationPriority {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum XcEventProfile {
    XcMarathon,
    TechnicalSingletrack,
    EnduranceMtb,
    UltraMtb,
    Custom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum XcReadinessStatus {
    OnTrack,
    Watch,
    FallingBehind,
    MissingData,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum XcReadinessGateKey {
    LongRideDistance,
    BigClimbDay,
    ClimbDensity,
    TargetFinishPace,
    AerobicDecoupling,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum XcTrainingDeficitKey {
    LongRide,
    BigClimbDay,
    EventSpecificity,
    FinishPace,
    AerobicDurability,
    Recovery,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum XcTrainingPurpose {
    BaseEndurance,
    ClimbDurability,
    Tempo,
    Threshold,
    PunchVo2,
    TechnicalFatigue,
    Recovery,
    DataQuality,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingGoalMetricResponse {
    pub key: TrainingGoalKey,
    pub label: String,
    pub unit: TrainingMetricUnit,
    pub direction: TrainingGoalDirection,
    pub current_value: Option<f64>,
    pub target_value: f64,
    pub progress_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TrainingRecommendationResponse {
    pub key: TrainingRecommendationKey,
    pub priority: TrainingRecommendationPriority,
    pub title: String,
    pub detail: String,
    pub purpose: Option<XcTrainingPurpose>,
    pub limiter: Option<String>,
    pub gap_value: Option<f64>,
    pub gap_unit: Option<TrainingMetricUnit>,
    pub suggested_ride: Option<XcSuggestedRideResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcProgressSummaryResponse {
    pub recent_window_days: i32,
    pub recent_ride_count: i32,
    pub comparable_ride_count: i32,
    pub total_z2_time_seconds: i32,
    pub total_climbing_time_seconds: i32,
    pub total_climbing_elevation_gain_meters: f64,
    pub average_aerobic_decoupling_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcRideProgressResponse {
    pub activity_id: i32,
    pub activity_title: String,
    pub started_at: DateTime<Utc>,
    pub activity_type: ActivityType,
    pub ride_focus: ActivityRideFocus,
    pub route_family_key: Option<String>,
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub z2_time_seconds: i32,
    pub z2_distance_meters: Option<f64>,
    pub z2_average_speed_mps: Option<f64>,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: Option<f64>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub z1_seconds: i32,
    pub z2_zone_seconds: i32,
    pub z3_seconds: i32,
    pub z4_seconds: i32,
    pub z5_seconds: i32,
    pub training_purpose: XcTrainingPurpose,
    pub training_purpose_detail: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcRaceResultResponse {
    pub activity_id: i32,
    pub activity_title: String,
    pub started_at: DateTime<Utc>,
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub average_speed_mps: Option<f64>,
    pub climb_density_meters_per_kilometer: Option<f64>,
    pub z2_time_seconds: i32,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: Option<f64>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub prior_training_ride_count: i32,
    pub prior_training_z2_time_seconds: i32,
    pub prior_training_climbing_elevation_gain_meters: f64,
    pub prior_training_average_z2_speed_mps: Option<f64>,
    pub prior_training_average_aerobic_decoupling_percent: Option<f64>,
    pub race_vs_best_training_distance_percent: Option<f64>,
    pub race_vs_best_training_elevation_percent: Option<f64>,
    pub insight_title: String,
    pub insight_detail: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcWeeklyProgressPointResponse {
    pub week_start: String,
    pub ride_count: i32,
    pub comparable_ride_count: i32,
    pub distance_meters: f64,
    pub z2_time_seconds: i32,
    pub z2_distance_meters: f64,
    pub average_z2_speed_mps: Option<f64>,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: f64,
    pub climbing_vertical_rate_meters_per_hour: Option<f64>,
    pub average_aerobic_decoupling_percent: Option<f64>,
    pub z1_seconds: i32,
    pub z2_zone_seconds: i32,
    pub z3_seconds: i32,
    pub z4_seconds: i32,
    pub z5_seconds: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcGoalProgressResponse {
    pub generated_at: DateTime<Utc>,
    pub event_goal: Option<XcEventGoalResponse>,
    pub readiness: Option<XcReadinessSummaryResponse>,
    pub deficits: Vec<XcTrainingDeficitResponse>,
    pub summary: XcProgressSummaryResponse,
    pub race_results: Vec<XcRaceResultResponse>,
    pub goals: Vec<TrainingGoalMetricResponse>,
    pub recommendations: Vec<TrainingRecommendationResponse>,
    pub weekly_progress: Vec<XcWeeklyProgressPointResponse>,
    pub recent_rides: Vec<XcRideProgressResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcEventGoalResponse {
    pub event_name: Option<String>,
    pub event_profile: Option<XcEventProfile>,
    pub start_date: String,
    pub target_date: String,
    pub days_remaining: i64,
    pub target_distance_meters: f64,
    pub target_elevation_gain_meters: f64,
    pub target_finish_time_seconds: Option<i32>,
    pub target_finish_speed_mps: Option<f64>,
    pub target_climb_density_meters_per_kilometer: f64,
    pub training_window_days: i32,
    pub counted_ride_count: i32,
    pub counted_distance_meters: f64,
    pub counted_elevation_gain_meters: f64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcReadinessSummaryResponse {
    pub status: XcReadinessStatus,
    pub title: String,
    pub reason: String,
    pub missing_most: Option<String>,
    pub gates: Vec<XcReadinessGateResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcReadinessGateResponse {
    pub key: XcReadinessGateKey,
    pub label: String,
    pub status: XcReadinessStatus,
    pub unit: TrainingMetricUnit,
    pub direction: TrainingGoalDirection,
    pub current_value: Option<f64>,
    pub target_value: Option<f64>,
    pub gap_value: Option<f64>,
    pub progress_percent: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcTrainingDeficitResponse {
    pub key: XcTrainingDeficitKey,
    pub priority: TrainingRecommendationPriority,
    pub title: String,
    pub detail: String,
    pub gap_value: Option<f64>,
    pub gap_unit: Option<TrainingMetricUnit>,
    pub suggested_ride: XcSuggestedRideResponse,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcSuggestedRideResponse {
    pub purpose: XcTrainingPurpose,
    pub duration_seconds_min: Option<i32>,
    pub duration_seconds_max: Option<i32>,
    pub distance_meters_min: Option<f64>,
    pub distance_meters_max: Option<f64>,
    pub climbing_elevation_gain_meters: Option<f64>,
    pub intensity: String,
    pub terrain: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DhProgressSummaryResponse {
    pub segment_count: i32,
    pub session_count: i32,
    pub effort_count: i32,
    pub average_efforts_per_session: Option<f64>,
    pub average_repeat_fade_percent: Option<f64>,
    pub average_top_3_gap_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DhSegmentProgressResponse {
    pub segment_id: i32,
    pub segment_title: String,
    pub effort_count: i32,
    pub personal_record_duration_seconds: Option<i32>,
    pub recent_best_duration_seconds: Option<i32>,
    pub rolling_top_3_average_duration_seconds: Option<f64>,
    pub top_3_pr_gap_percent: Option<f64>,
    pub repeat_fade_percent: Option<f64>,
    pub latest_activity_id: Option<i32>,
    pub latest_activity_title: Option<String>,
    pub latest_activity_started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DhSessionSummaryResponse {
    pub activity_id: i32,
    pub activity_title: String,
    pub started_at: DateTime<Utc>,
    pub segment_count: i32,
    pub effort_count: i32,
    pub fastest_effort_duration_seconds: Option<i32>,
    pub average_repeat_fade_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct DhGoalProgressResponse {
    pub generated_at: DateTime<Utc>,
    pub summary: DhProgressSummaryResponse,
    pub goals: Vec<TrainingGoalMetricResponse>,
    pub recommendations: Vec<TrainingRecommendationResponse>,
    pub segments: Vec<DhSegmentProgressResponse>,
    pub recent_sessions: Vec<DhSessionSummaryResponse>,
}

#[utoipa::path(
    get,
    path = "/api/training/xc-progress",
    responses(
        (status = 200, description = "XC goals and progress summary for the authenticated user", body = XcGoalProgressResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_xc_goal_progress(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<XcGoalProgressResponse>, AppError> {
    Ok(Json(
        TrainingGoalsService::xc_goal_progress(&state.db, user.id, Utc::now()).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/training/dh-progress",
    responses(
        (status = 200, description = "DH goals and progress summary for the authenticated user", body = DhGoalProgressResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "training",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_dh_goal_progress(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<DhGoalProgressResponse>, AppError> {
    Ok(Json(
        TrainingGoalsService::dh_goal_progress(&state.db, user.id, Utc::now()).await?,
    ))
}
