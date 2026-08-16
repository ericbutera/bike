use crate::activity_training_analysis::{plausible_aerobic_decoupling_percent, ActivityRideFocus};
use crate::activity_type::ActivityType;
use crate::analytics::{FATIGUE_WINDOW_DAYS, FITNESS_WINDOW_DAYS};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{
    activities, activity_training_analyses, analytics_user_states, fitness_freshness_daily,
    segment_efforts, segments, user_preferences,
};
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_activity_heart_rate_zones, StoredActivityHeartRateZones,
};
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;
use utoipa::ToSchema;

const XC_RECENT_WINDOW_DAYS: i64 = 28;
const XC_DECOUPLING_WINDOW_DAYS: i64 = 90;
const XC_BENCHMARK_WINDOW_DAYS: i64 = 90;
const XC_WEEKLY_PROGRESS_WEEKS: i64 = 8;
const XC_RECENT_RIDES_LIMIT: usize = 12;
const XC_WEEKLY_Z2_GOAL_SECONDS: f64 = 14_400.0;
const XC_WEEKLY_CLIMBING_GOAL_METERS: f64 = 1_500.0;
const XC_AEROBIC_DECOUPLING_GOAL_PERCENT: f64 = 5.0;
const XC_LONG_RIDE_TARGET_RATIO: f64 = 0.65;
const XC_BIG_CLIMB_DAY_TARGET_RATIO: f64 = 0.45;
const XC_EVENT_TARGET_ELEVATION_GAIN_MAX_METERS: f64 = 25_000.0 * 0.3048;
const XC_CLIMB_DENSITY_READY_RATIO: f64 = 0.8;
const XC_CLIMB_DENSITY_WATCH_RATIO: f64 = 0.6;
const XC_FINISH_SPEED_READY_RATIO: f64 = 1.05;
const XC_FINISH_SPEED_WATCH_RATIO: f64 = 0.9;
const XC_TAPER_WINDOW_DAYS: i64 = 14;

const DH_RECENT_SESSION_LIMIT: usize = 12;
const DH_RECENT_EFFORT_LIMIT: usize = 5;
const DH_RECENT_FADE_SESSION_LIMIT: usize = 3;
const DH_LAPS_PER_SESSION_GOAL: f64 = 3.0;
const DH_REPEAT_FADE_GOAL_PERCENT: f64 = 5.0;
const DH_TOP3_GAP_GOAL_PERCENT: f64 = 3.0;

const FRESHNESS_RECOVERY_FORM_THRESHOLD: f64 = -10.0;
const FRESHNESS_RECOVERY_FATIGUE_MARGIN: f64 = 8.0;
const FRESHNESS_READY_FORM_THRESHOLD: f64 = 5.0;
const FRESHNESS_READY_FATIGUE_MARGIN: f64 = 3.0;
const XC_READY_FITNESS_THRESHOLD: f64 = 25.0;
const DH_READY_FITNESS_THRESHOLD: f64 = 12.0;

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

impl XcEventProfile {
    fn from_stored(value: Option<&str>) -> Option<Self> {
        match value {
            Some("xc_marathon") => Some(Self::XcMarathon),
            Some("technical_singletrack") => Some(Self::TechnicalSingletrack),
            Some("endurance_mtb") => Some(Self::EnduranceMtb),
            Some("ultra_mtb") => Some(Self::UltraMtb),
            Some("custom") => Some(Self::Custom),
            _ => None,
        }
    }
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

#[derive(Debug, Clone)]
struct XcEventGoal {
    start_date: NaiveDate,
    target_date: NaiveDate,
    target_distance_meters: f64,
    target_elevation_gain_meters: f64,
    event_name: Option<String>,
    target_finish_time_seconds: Option<i32>,
    event_profile: Option<XcEventProfile>,
}

#[derive(Debug, Clone, Copy)]
struct FitnessFreshnessSnapshot {
    day: NaiveDate,
    fitness: f64,
    fatigue: f64,
    form: f64,
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

#[derive(Debug, Clone)]
struct DhEffortSource {
    segment_id: i32,
    activity_id: i32,
    activity_title: String,
    started_at: DateTime<Utc>,
    effort_index: i32,
    duration_seconds: i32,
}

#[derive(Clone, Debug, FromQueryResult)]
struct ActivitySummaryRow {
    id: i32,
    title: String,
    started_at: DateTime<Utc>,
    distance_meters: Option<f64>,
    elevation_gain_meters: Option<f64>,
    moving_time_seconds: Option<i32>,
    total_time_seconds: Option<i32>,
    activity_type: String,
    heart_rate_zones_json: Option<StoredActivityHeartRateZones>,
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
    let goal = load_xc_event_goal(&state.db, user.id).await?;
    let freshness =
        load_latest_fitness_freshness_snapshot(&state.db, user.id, Utc::now().date_naive()).await?;
    let analysis_models = activity_training_analyses::Entity::find()
        .filter(activity_training_analyses::Column::UserId.eq(user.id))
        .all(&state.db)
        .await?;
    let activity_ids = analysis_models
        .iter()
        .map(|analysis| analysis.activity_id)
        .collect::<Vec<_>>();
    let activity_by_id = load_activity_summaries_by_ids(&state.db, user.id, &activity_ids).await?;

    let mut rides = analysis_models
        .into_iter()
        .filter_map(|analysis| {
            let activity = activity_by_id.get(&analysis.activity_id)?;
            let ride_focus = ActivityRideFocus::from_stored(&analysis.ride_focus);
            let zone_seconds = heart_rate_zone_seconds(activity);

            Some(XcRideProgressResponse {
                activity_id: activity.id,
                activity_title: activity.title.clone(),
                started_at: activity.started_at,
                activity_type: ActivityType::from_stored(&activity.activity_type),
                ride_focus,
                route_family_key: analysis.route_family_key,
                distance_meters: activity.distance_meters,
                elevation_gain_meters: activity.elevation_gain_meters,
                moving_time_seconds: activity.moving_time_seconds.or(activity.total_time_seconds),
                z2_time_seconds: analysis.z2_time_seconds,
                z2_distance_meters: analysis.z2_distance_meters,
                z2_average_speed_mps: analysis.z2_average_speed_mps,
                climbing_time_seconds: analysis.climbing_time_seconds,
                climbing_elevation_gain_meters: analysis
                    .climbing_elevation_gain_meters
                    .or(activity.elevation_gain_meters),
                aerobic_decoupling_percent: analysis
                    .aerobic_decoupling_percent
                    .and_then(plausible_aerobic_decoupling_percent),
                z1_seconds: zone_seconds[0],
                z2_zone_seconds: zone_seconds[1],
                z3_seconds: zone_seconds[2],
                z4_seconds: zone_seconds[3],
                z5_seconds: zone_seconds[4],
                training_purpose: XcTrainingPurpose::DataQuality,
                training_purpose_detail:
                    "Training purpose will be classified with the active event target.".to_string(),
            })
        })
        .collect::<Vec<_>>();
    let ride_ids = rides
        .iter()
        .map(|ride| ride.activity_id)
        .collect::<HashSet<_>>();
    let race_activities = load_race_activity_summaries(&state.db, user.id).await?;

    rides.extend(
        race_activities
            .into_iter()
            .filter(|activity| !ride_ids.contains(&activity.id))
            .map(race_activity_without_analysis),
    );

    rides.sort_by(|left, right| right.started_at.cmp(&left.started_at));

    Ok(Json(build_xc_goal_progress_response(
        rides,
        goal,
        freshness,
        Utc::now(),
    )))
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
    let freshness =
        load_latest_fitness_freshness_snapshot(&state.db, user.id, Utc::now().date_naive()).await?;
    let segment_models = segments::Entity::find()
        .filter(segments::Column::UserId.eq(user.id))
        .filter(segments::Column::Mode.eq("dh"))
        .all(&state.db)
        .await?;

    let segment_ids = segment_models
        .iter()
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    let effort_models = if segment_ids.is_empty() {
        Vec::new()
    } else {
        segment_efforts::Entity::find()
            .filter(segment_efforts::Column::UserId.eq(user.id))
            .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.clone()))
            .all(&state.db)
            .await?
    };
    let activity_ids = effort_models
        .iter()
        .map(|effort| effort.activity_id)
        .collect::<Vec<_>>();
    let activity_by_id = load_activity_summaries_by_ids(&state.db, user.id, &activity_ids).await?;
    let efforts = effort_models
        .into_iter()
        .filter_map(|effort| {
            let activity = activity_by_id.get(&effort.activity_id)?;

            Some(DhEffortSource {
                segment_id: effort.segment_id,
                activity_id: activity.id,
                activity_title: activity.title.clone(),
                started_at: activity.started_at,
                effort_index: effort.effort_index,
                duration_seconds: effort.duration_seconds,
            })
        })
        .collect::<Vec<_>>();

    Ok(Json(build_dh_goal_progress_response(
        segment_models,
        efforts,
        freshness,
        Utc::now(),
    )))
}

async fn load_race_activity_summaries(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
) -> Result<Vec<ActivitySummaryRow>, AppError> {
    Ok(activities::Entity::find()
        .select_only()
        .column(activities::Column::Id)
        .column(activities::Column::Title)
        .column(activities::Column::StartedAt)
        .column(activities::Column::DistanceMeters)
        .column(activities::Column::ElevationGainMeters)
        .column(activities::Column::MovingTimeSeconds)
        .column(activities::Column::TotalTimeSeconds)
        .column(activities::Column::ActivityType)
        .column(activities::Column::HeartRateZonesJson)
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::ActivityType.eq(ActivityType::Race.as_str()))
        .into_model::<ActivitySummaryRow>()
        .all(db)
        .await?)
}

fn race_activity_without_analysis(activity: ActivitySummaryRow) -> XcRideProgressResponse {
    let zone_seconds = heart_rate_zone_seconds(&activity);

    XcRideProgressResponse {
        activity_id: activity.id,
        activity_title: activity.title,
        started_at: activity.started_at,
        activity_type: ActivityType::from_stored(&activity.activity_type),
        ride_focus: ActivityRideFocus::XcEndurance,
        route_family_key: None,
        distance_meters: activity.distance_meters,
        elevation_gain_meters: activity.elevation_gain_meters,
        moving_time_seconds: activity.moving_time_seconds.or(activity.total_time_seconds),
        z2_time_seconds: 0,
        z2_distance_meters: None,
        z2_average_speed_mps: None,
        climbing_time_seconds: 0,
        climbing_elevation_gain_meters: activity.elevation_gain_meters,
        aerobic_decoupling_percent: None,
        z1_seconds: zone_seconds[0],
        z2_zone_seconds: zone_seconds[1],
        z3_seconds: zone_seconds[2],
        z4_seconds: zone_seconds[3],
        z5_seconds: zone_seconds[4],
        training_purpose: XcTrainingPurpose::DataQuality,
        training_purpose_detail:
            "Race result is used as an outcome benchmark, not a training prescription.".to_string(),
    }
}

async fn load_activity_summaries_by_ids(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<HashMap<i32, ActivitySummaryRow>, AppError> {
    if activity_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(activities::Entity::find()
        .select_only()
        .column(activities::Column::Id)
        .column(activities::Column::Title)
        .column(activities::Column::StartedAt)
        .column(activities::Column::DistanceMeters)
        .column(activities::Column::ElevationGainMeters)
        .column(activities::Column::MovingTimeSeconds)
        .column(activities::Column::TotalTimeSeconds)
        .column(activities::Column::ActivityType)
        .column(activities::Column::HeartRateZonesJson)
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .into_model::<ActivitySummaryRow>()
        .all(db)
        .await?
        .into_iter()
        .map(|activity| (activity.id, activity))
        .collect())
}

fn heart_rate_zone_seconds(activity: &ActivitySummaryRow) -> [i32; 5] {
    let mut zone_seconds = [0; 5];

    for zone in deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref()) {
        let zone_index = (zone.zone - 1).clamp(0, 4) as usize;
        zone_seconds[zone_index] += zone.duration_seconds.max(0);
    }

    zone_seconds
}

async fn load_xc_event_goal(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
) -> Result<Option<XcEventGoal>, AppError> {
    let preferences = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user_id))
        .one(db)
        .await?;
    let Some(preferences) = preferences else {
        return Ok(None);
    };

    match (
        preferences.xc_goal_start_date,
        preferences.xc_goal_target_date,
        preferences.xc_goal_target_distance_meters,
        preferences.xc_goal_target_elevation_gain_meters,
    ) {
        (
            Some(start_date),
            Some(target_date),
            Some(target_distance_meters),
            Some(target_elevation_gain_meters),
        ) if start_date <= target_date
            && target_distance_meters > 0.0
            && target_elevation_gain_meters > 0.0 =>
        {
            Ok(Some(XcEventGoal {
                start_date,
                target_date,
                target_distance_meters,
                target_elevation_gain_meters,
                event_name: preferences.xc_goal_event_name,
                target_finish_time_seconds: preferences.xc_goal_target_finish_time_seconds,
                event_profile: XcEventProfile::from_stored(
                    preferences.xc_goal_event_profile.as_deref(),
                ),
            }))
        }
        _ => Ok(None),
    }
}

async fn load_latest_fitness_freshness_snapshot(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    end_date: NaiveDate,
) -> Result<Option<FitnessFreshnessSnapshot>, AppError> {
    let freshness_state = analytics_user_states::Entity::find_by_id(user_id)
        .one(db)
        .await?;

    if freshness_state
        .as_ref()
        .and_then(|state| state.fitness_dirty_from_day)
        .is_some_and(|dirty_from_day| dirty_from_day <= end_date)
    {
        return Ok(None);
    }

    let latest_row = fitness_freshness_daily::Entity::find()
        .filter(fitness_freshness_daily::Column::UserId.eq(user_id))
        .filter(fitness_freshness_daily::Column::Day.lte(end_date))
        .order_by_desc(fitness_freshness_daily::Column::Day)
        .one(db)
        .await?;

    Ok(latest_row.map(|row| {
        decay_fitness_freshness_snapshot_to_day(
            FitnessFreshnessSnapshot {
                day: row.day,
                fitness: row.fitness,
                fatigue: row.fatigue,
                form: row.form,
            },
            end_date,
        )
    }))
}

fn decay_fitness_freshness_snapshot_to_day(
    mut snapshot: FitnessFreshnessSnapshot,
    end_date: NaiveDate,
) -> FitnessFreshnessSnapshot {
    while snapshot.day < end_date {
        snapshot.day += Duration::days(1);
        snapshot.fitness += (0.0 - snapshot.fitness) / FITNESS_WINDOW_DAYS;
        snapshot.fatigue += (0.0 - snapshot.fatigue) / FATIGUE_WINDOW_DAYS;
        snapshot.form = snapshot.fitness - snapshot.fatigue;
    }

    snapshot
}

fn build_xc_goal_progress_response(
    mut rides: Vec<XcRideProgressResponse>,
    goal: Option<XcEventGoal>,
    freshness: Option<FitnessFreshnessSnapshot>,
    now: DateTime<Utc>,
) -> XcGoalProgressResponse {
    rides.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    for ride in &mut rides {
        ride.aerobic_decoupling_percent = ride
            .aerobic_decoupling_percent
            .and_then(plausible_aerobic_decoupling_percent);
        apply_xc_training_purpose(ride, goal.as_ref());
    }

    let race_results = build_xc_race_results(&rides);
    let training_rides = rides
        .iter()
        .filter(|ride| !ride.activity_type.is_race())
        .cloned()
        .collect::<Vec<_>>();

    let event_goal = goal
        .as_ref()
        .and_then(|goal| build_xc_event_goal_response(&training_rides, goal, now));
    let history_end = goal
        .as_ref()
        .map(|goal| std::cmp::min(now.date_naive(), goal.target_date));

    let recent_window_start = now - Duration::days(XC_RECENT_WINDOW_DAYS);
    let decoupling_window_start = now - Duration::days(XC_DECOUPLING_WINDOW_DAYS);
    let recent_rides = training_rides
        .iter()
        .filter(|ride| ride.started_at >= recent_window_start)
        .collect::<Vec<_>>();
    let recent_comparable_rides = recent_rides
        .iter()
        .filter(|ride| ride.aerobic_decoupling_percent.is_some())
        .copied()
        .collect::<Vec<_>>();
    let recent_decoupling_average = average_f64(
        training_rides
            .iter()
            .filter(|ride| ride.started_at >= decoupling_window_start)
            .filter_map(|ride| ride.aerobic_decoupling_percent),
    );
    let total_z2_time_seconds = recent_rides
        .iter()
        .map(|ride| ride.z2_time_seconds)
        .sum::<i32>();
    let total_climbing_time_seconds = recent_rides
        .iter()
        .map(|ride| ride.climbing_time_seconds)
        .sum::<i32>();
    let total_climbing_elevation_gain_meters = recent_rides
        .iter()
        .filter_map(|ride| ride.climbing_elevation_gain_meters)
        .sum::<f64>();
    let weekly_divisor = XC_RECENT_WINDOW_DAYS as f64 / 7.0;
    let weekly_z2_average_seconds = f64::from(total_z2_time_seconds) / weekly_divisor;
    let weekly_climbing_average_meters = total_climbing_elevation_gain_meters / weekly_divisor;

    let summary = XcProgressSummaryResponse {
        recent_window_days: XC_RECENT_WINDOW_DAYS as i32,
        recent_ride_count: recent_rides.len() as i32,
        comparable_ride_count: recent_comparable_rides.len() as i32,
        total_z2_time_seconds,
        total_climbing_time_seconds,
        total_climbing_elevation_gain_meters: round_metric(total_climbing_elevation_gain_meters),
        average_aerobic_decoupling_percent: recent_decoupling_average.map(round_metric),
    };

    let goals = vec![
        build_goal_metric(
            TrainingGoalKey::WeeklyZ2Average,
            "Weekly Z2 average",
            TrainingMetricUnit::Seconds,
            TrainingGoalDirection::AtLeast,
            Some(weekly_z2_average_seconds),
            XC_WEEKLY_Z2_GOAL_SECONDS,
        ),
        build_goal_metric(
            TrainingGoalKey::WeeklyClimbingAverage,
            "Weekly climbing average",
            TrainingMetricUnit::Meters,
            TrainingGoalDirection::AtLeast,
            Some(weekly_climbing_average_meters),
            XC_WEEKLY_CLIMBING_GOAL_METERS,
        ),
        build_goal_metric(
            TrainingGoalKey::AerobicDecoupling,
            "Aerobic decoupling",
            TrainingMetricUnit::Percent,
            TrainingGoalDirection::AtMost,
            recent_decoupling_average,
            XC_AEROBIC_DECOUPLING_GOAL_PERCENT,
        ),
    ];
    let (readiness, deficits) = goal
        .as_ref()
        .map(|goal| build_xc_readiness(&rides, goal, now, recent_decoupling_average, freshness))
        .unwrap_or((None, Vec::new()));
    let recommendations = if goal.is_some() {
        build_xc_event_recommendations(
            &deficits,
            &training_rides,
            recent_comparable_rides.len(),
            weekly_z2_average_seconds,
            weekly_climbing_average_meters,
            recent_decoupling_average,
            freshness,
        )
    } else {
        build_xc_recommendations(
            &training_rides,
            recent_comparable_rides.len(),
            weekly_z2_average_seconds,
            weekly_climbing_average_meters,
            recent_decoupling_average,
            freshness,
        )
    };
    let weekly_progress = build_xc_weekly_progress(&training_rides, now, goal.as_ref());
    let recent_rides = training_rides
        .into_iter()
        .filter(|ride| match (goal.as_ref(), history_end) {
            (Some(goal), Some(history_end)) => {
                let ride_day = ride.started_at.date_naive();
                ride_day >= goal.start_date && ride_day <= history_end
            }
            _ => true,
        })
        .take(XC_RECENT_RIDES_LIMIT)
        .collect();

    XcGoalProgressResponse {
        generated_at: now,
        event_goal,
        readiness,
        deficits,
        summary,
        race_results,
        goals,
        recommendations,
        weekly_progress,
        recent_rides,
    }
}

fn build_xc_event_goal_response(
    rides: &[XcRideProgressResponse],
    goal: &XcEventGoal,
    now: DateTime<Utc>,
) -> Option<XcEventGoalResponse> {
    if goal.start_date > goal.target_date {
        return None;
    }

    let progress_end = std::cmp::min(now.date_naive(), goal.target_date);
    let counted_rides = rides
        .iter()
        .filter(|ride| {
            let ride_day = ride.started_at.date_naive();
            ride_day >= goal.start_date && ride_day <= progress_end
        })
        .collect::<Vec<_>>();
    let counted_distance_meters = counted_rides
        .iter()
        .map(|ride| ride.distance_meters.unwrap_or_default())
        .sum::<f64>();
    let counted_elevation_gain_meters = counted_rides
        .iter()
        .map(|ride| {
            ride.climbing_elevation_gain_meters
                .or(ride.elevation_gain_meters)
                .unwrap_or_default()
        })
        .sum::<f64>();
    Some(XcEventGoalResponse {
        event_name: goal.event_name.clone(),
        event_profile: goal.event_profile,
        start_date: goal.start_date.format("%Y-%m-%d").to_string(),
        target_date: goal.target_date.format("%Y-%m-%d").to_string(),
        days_remaining: goal
            .target_date
            .signed_duration_since(now.date_naive())
            .num_days(),
        target_distance_meters: round_metric(goal.target_distance_meters),
        target_elevation_gain_meters: round_metric(goal.target_elevation_gain_meters),
        target_finish_time_seconds: goal.target_finish_time_seconds,
        target_finish_speed_mps: goal
            .target_finish_time_seconds
            .and_then(|seconds| target_finish_speed_mps(goal.target_distance_meters, seconds))
            .map(round_metric),
        target_climb_density_meters_per_kilometer: round_metric(
            climb_density_meters_per_kilometer(
                Some(goal.target_elevation_gain_meters),
                Some(goal.target_distance_meters),
            )
            .unwrap_or_default(),
        ),
        training_window_days: (goal
            .target_date
            .signed_duration_since(goal.start_date)
            .num_days()
            + 1) as i32,
        counted_ride_count: counted_rides.len() as i32,
        counted_distance_meters: round_metric(counted_distance_meters),
        counted_elevation_gain_meters: round_metric(counted_elevation_gain_meters),
    })
}

fn build_xc_readiness(
    rides: &[XcRideProgressResponse],
    goal: &XcEventGoal,
    now: DateTime<Utc>,
    recent_decoupling_average: Option<f64>,
    freshness: Option<FitnessFreshnessSnapshot>,
) -> (
    Option<XcReadinessSummaryResponse>,
    Vec<XcTrainingDeficitResponse>,
) {
    let progress_end = std::cmp::min(now.date_naive(), goal.target_date);
    if goal.start_date > goal.target_date {
        return (None, Vec::new());
    }

    let counted_rides = rides
        .iter()
        .filter(|ride| {
            let ride_day = ride.started_at.date_naive();
            ride_day >= goal.start_date && ride_day <= progress_end
        })
        .collect::<Vec<_>>();
    let training_counted_rides = counted_rides
        .iter()
        .copied()
        .filter(|ride| !ride.activity_type.is_race())
        .collect::<Vec<_>>();
    let counted_distance_meters = training_counted_rides
        .iter()
        .map(|ride| ride.distance_meters.unwrap_or_default())
        .sum::<f64>();
    let counted_climbing_meters = training_counted_rides
        .iter()
        .filter_map(|ride| effective_climbing_elevation_gain_meters(ride))
        .sum::<f64>();
    let benchmark_window_start = now - Duration::days(XC_BENCHMARK_WINDOW_DAYS);
    let benchmark_rides = counted_rides
        .iter()
        .copied()
        .filter(|ride| ride.activity_type.is_race() || ride.started_at >= benchmark_window_start)
        .collect::<Vec<_>>();
    let best_distance_meters = benchmark_rides
        .iter()
        .filter_map(|ride| ride.distance_meters)
        .reduce(f64::max);
    let best_climbing_meters = benchmark_rides
        .iter()
        .filter_map(|ride| effective_climbing_elevation_gain_meters(ride))
        .reduce(f64::max);
    let current_climb_density = climb_density_meters_per_kilometer(
        Some(counted_climbing_meters),
        Some(counted_distance_meters),
    );
    let readiness_target_elevation_gain_meters = readiness_goal_elevation_gain_meters(goal);
    let target_climb_density = climb_density_meters_per_kilometer(
        Some(readiness_target_elevation_gain_meters),
        Some(goal.target_distance_meters),
    );
    let target_finish_speed = goal
        .target_finish_time_seconds
        .and_then(|seconds| target_finish_speed_mps(goal.target_distance_meters, seconds));
    let recent_window_start = now - Duration::days(XC_RECENT_WINDOW_DAYS);
    let recent_rides = rides
        .iter()
        .filter(|ride| ride.started_at >= recent_window_start)
        .collect::<Vec<_>>();
    let recent_z2_speed = weighted_z2_speed_mps(recent_rides.iter().copied());

    let mut gates = vec![
        build_at_least_gate(
            XcReadinessGateKey::LongRideDistance,
            "Recent long ride",
            best_distance_meters,
            goal.target_distance_meters * XC_LONG_RIDE_TARGET_RATIO,
            TrainingMetricUnit::Meters,
            1.0,
            0.75,
            "Best single ride in the last 90 days, or any race in the saved block, compared with the long-ride benchmark for this event distance.",
        ),
        build_at_least_gate(
            XcReadinessGateKey::BigClimbDay,
            "Recent climb day",
            best_climbing_meters,
            big_climb_day_target_meters(goal),
            TrainingMetricUnit::Meters,
            1.0,
            0.75,
            "Best single climbing ride in the last 90 days, or any race in the saved block, compared with the climbing-day benchmark for this event.",
        ),
        build_at_least_gate(
            XcReadinessGateKey::ClimbDensity,
            "Climb density",
            current_climb_density,
            target_climb_density.unwrap_or_default() * XC_CLIMB_DENSITY_READY_RATIO,
            TrainingMetricUnit::MetersPerKilometer,
            1.0,
            XC_CLIMB_DENSITY_WATCH_RATIO / XC_CLIMB_DENSITY_READY_RATIO,
            "Training-block climbing per distance compared with the event's climb density.",
        ),
        build_at_most_gate(
            XcReadinessGateKey::AerobicDecoupling,
            "Aerobic decoupling",
            recent_decoupling_average,
            XC_AEROBIC_DECOUPLING_GOAL_PERCENT,
            TrainingMetricUnit::Percent,
            1.0,
            1.4,
            "Recent comparable endurance drift. Lower is better.",
        ),
        build_recovery_gate(freshness),
    ];

    if let Some(target_finish_speed) = target_finish_speed {
        gates.push(build_at_least_gate(
            XcReadinessGateKey::TargetFinishPace,
            "Target finish pace",
            recent_z2_speed,
            target_finish_speed * XC_FINISH_SPEED_READY_RATIO,
            TrainingMetricUnit::MetersPerSecond,
            1.0,
            XC_FINISH_SPEED_WATCH_RATIO / XC_FINISH_SPEED_READY_RATIO,
            "Recent Z2 speed compared with the event elapsed-speed target plus a small buffer.",
        ));
    }

    let mut deficits = build_xc_deficits_from_gates(&gates, goal, now);
    deficits.sort_by(|left, right| {
        recommendation_priority_rank(left.priority)
            .cmp(&recommendation_priority_rank(right.priority))
            .then_with(|| deficit_rank(left.key).cmp(&deficit_rank(right.key)))
    });

    let status = overall_readiness_status(&gates);
    let missing_most = deficits.first().map(|deficit| deficit.title.clone());
    let title = match status {
        XcReadinessStatus::OnTrack => "On track".to_string(),
        XcReadinessStatus::Watch => "Watch the gaps".to_string(),
        XcReadinessStatus::FallingBehind => "Falling behind".to_string(),
        XcReadinessStatus::MissingData => "Need more data".to_string(),
    };
    let reason = readiness_reason(status, &gates, &deficits);

    (
        Some(XcReadinessSummaryResponse {
            status,
            title,
            reason,
            missing_most,
            gates,
        }),
        deficits,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "readiness gate builder mirrors the API response fields"
)]
fn build_at_least_gate(
    key: XcReadinessGateKey,
    label: &str,
    current_value: Option<f64>,
    target_value: f64,
    unit: TrainingMetricUnit,
    on_track_ratio: f64,
    watch_ratio: f64,
    detail: &str,
) -> XcReadinessGateResponse {
    let status = match current_value {
        Some(current) if target_value > 0.0 && current >= target_value * on_track_ratio => {
            XcReadinessStatus::OnTrack
        }
        Some(current) if target_value > 0.0 && current >= target_value * watch_ratio => {
            XcReadinessStatus::Watch
        }
        Some(_) => XcReadinessStatus::FallingBehind,
        None => XcReadinessStatus::MissingData,
    };

    XcReadinessGateResponse {
        key,
        label: label.to_string(),
        status,
        unit,
        direction: TrainingGoalDirection::AtLeast,
        current_value: current_value.map(round_metric),
        target_value: Some(round_metric(target_value)),
        gap_value: current_value
            .map(|current| (target_value - current).max(0.0))
            .map(round_metric),
        progress_percent: goal_progress_percent(
            current_value,
            target_value,
            TrainingGoalDirection::AtLeast,
        )
        .map(round_metric),
        detail: detail.to_string(),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "readiness gate builder mirrors the API response fields"
)]
fn build_at_most_gate(
    key: XcReadinessGateKey,
    label: &str,
    current_value: Option<f64>,
    target_value: f64,
    unit: TrainingMetricUnit,
    on_track_ratio: f64,
    watch_ratio: f64,
    detail: &str,
) -> XcReadinessGateResponse {
    let status = match current_value {
        Some(current) if target_value > 0.0 && current <= target_value * on_track_ratio => {
            XcReadinessStatus::OnTrack
        }
        Some(current) if target_value > 0.0 && current <= target_value * watch_ratio => {
            XcReadinessStatus::Watch
        }
        Some(_) => XcReadinessStatus::FallingBehind,
        None => XcReadinessStatus::MissingData,
    };

    XcReadinessGateResponse {
        key,
        label: label.to_string(),
        status,
        unit,
        direction: TrainingGoalDirection::AtMost,
        current_value: current_value.map(round_metric),
        target_value: Some(round_metric(target_value)),
        gap_value: current_value
            .map(|current| (current - target_value).max(0.0))
            .map(round_metric),
        progress_percent: goal_progress_percent(
            current_value,
            target_value,
            TrainingGoalDirection::AtMost,
        )
        .map(round_metric),
        detail: detail.to_string(),
    }
}

fn build_recovery_gate(freshness: Option<FitnessFreshnessSnapshot>) -> XcReadinessGateResponse {
    let status = match freshness {
        Some(snapshot) if freshness_needs_recovery(snapshot) => XcReadinessStatus::FallingBehind,
        Some(snapshot) if freshness_is_ready_for_quality(snapshot, XC_READY_FITNESS_THRESHOLD) => {
            XcReadinessStatus::OnTrack
        }
        Some(_) => XcReadinessStatus::Watch,
        None => XcReadinessStatus::MissingData,
    };

    XcReadinessGateResponse {
        key: XcReadinessGateKey::Recovery,
        label: "Recovery".to_string(),
        status,
        unit: TrainingMetricUnit::Count,
        direction: TrainingGoalDirection::AtLeast,
        current_value: freshness.map(|snapshot| round_metric(snapshot.form)),
        target_value: Some(FRESHNESS_READY_FORM_THRESHOLD),
        gap_value: None,
        progress_percent: None,
        detail: match status {
            XcReadinessStatus::FallingBehind => {
                "Fatigue is outrunning fitness; make the next ride easier before adding load."
            }
            XcReadinessStatus::OnTrack => {
                "Fitness is established and form is positive enough for a quality benchmark."
            }
            XcReadinessStatus::Watch => {
                "Recovery is usable, but not a green light for stacking every deficit at once."
            }
            XcReadinessStatus::MissingData => {
                "Fitness/fatigue/form is not available, so recovery cannot adjust the advice."
            }
        }
        .to_string(),
    }
}

fn build_xc_deficits_from_gates(
    gates: &[XcReadinessGateResponse],
    goal: &XcEventGoal,
    now: DateTime<Utc>,
) -> Vec<XcTrainingDeficitResponse> {
    gates
        .iter()
        .filter(|gate| {
            matches!(
                gate.status,
                XcReadinessStatus::Watch | XcReadinessStatus::FallingBehind
            )
        })
        .map(|gate| deficit_from_gate(gate, goal, now))
        .collect()
}

fn deficit_from_gate(
    gate: &XcReadinessGateResponse,
    goal: &XcEventGoal,
    now: DateTime<Utc>,
) -> XcTrainingDeficitResponse {
    let priority = if gate.status == XcReadinessStatus::FallingBehind {
        TrainingRecommendationPriority::High
    } else {
        TrainingRecommendationPriority::Medium
    };
    let days_to_goal = goal
        .target_date
        .signed_duration_since(now.date_naive())
        .num_days();
    let in_taper_window = days_to_goal <= XC_TAPER_WINDOW_DAYS;

    match gate.key {
        XcReadinessGateKey::LongRideDistance => XcTrainingDeficitResponse {
            key: XcTrainingDeficitKey::LongRide,
            priority,
            title: "Long-ride benchmark is short".to_string(),
            detail: "Your biggest recent ride is still below the long endurance benchmark for this event distance.".to_string(),
            gap_value: gate.gap_value,
            gap_unit: Some(gate.unit),
            suggested_ride: suggested_long_ride(goal, in_taper_window),
        },
        XcReadinessGateKey::BigClimbDay => XcTrainingDeficitResponse {
            key: XcTrainingDeficitKey::BigClimbDay,
            priority,
            title: "Big climbing day is short".to_string(),
            detail: "Your biggest climbing ride is below the single-day climbing benchmark for this event.".to_string(),
            gap_value: gate.gap_value,
            gap_unit: Some(gate.unit),
            suggested_ride: suggested_climbing_ride(goal, in_taper_window),
        },
        XcReadinessGateKey::ClimbDensity => XcTrainingDeficitResponse {
            key: XcTrainingDeficitKey::EventSpecificity,
            priority,
            title: "Training routes are not specific enough".to_string(),
            detail: "Current rides are not matching the event's climbing per mile/kilometer closely enough.".to_string(),
            gap_value: gate.gap_value,
            gap_unit: Some(gate.unit),
            suggested_ride: suggested_specificity_ride(goal, in_taper_window),
        },
        XcReadinessGateKey::TargetFinishPace => XcTrainingDeficitResponse {
            key: XcTrainingDeficitKey::FinishPace,
            priority,
            title: "Finish-time pace needs work".to_string(),
            detail: "Recent Z2 speed is not comfortably above the target elapsed finish speed.".to_string(),
            gap_value: gate.gap_value,
            gap_unit: Some(gate.unit),
            suggested_ride: suggested_tempo_ride(goal, in_taper_window),
        },
        XcReadinessGateKey::AerobicDecoupling => XcTrainingDeficitResponse {
            key: XcTrainingDeficitKey::AerobicDurability,
            priority,
            title: "Aerobic durability is drifting".to_string(),
            detail: "Comparable endurance rides are fading more than the durability target.".to_string(),
            gap_value: gate.gap_value,
            gap_unit: Some(gate.unit),
            suggested_ride: suggested_durability_ride(goal, in_taper_window),
        },
        XcReadinessGateKey::Recovery => XcTrainingDeficitResponse {
            key: XcTrainingDeficitKey::Recovery,
            priority: TrainingRecommendationPriority::High,
            title: "Recovery should guide the next ride".to_string(),
            detail: "The next ride should absorb load before chasing the event gaps.".to_string(),
            gap_value: None,
            gap_unit: None,
            suggested_ride: suggested_recovery_ride(),
        },
    }
}

fn suggested_long_ride(goal: &XcEventGoal, in_taper_window: bool) -> XcSuggestedRideResponse {
    if in_taper_window {
        return suggested_taper_ride();
    }

    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::BaseEndurance,
        duration_seconds_min: Some(14_400),
        duration_seconds_max: Some(25_200),
        distance_meters_min: Some(goal.target_distance_meters * 0.4),
        distance_meters_max: Some(goal.target_distance_meters * XC_LONG_RIDE_TARGET_RATIO),
        climbing_elevation_gain_meters: Some(readiness_goal_elevation_gain_meters(goal) * 0.3),
        intensity: "Z2 with a conservative finish".to_string(),
        terrain: "MTB route that resembles the event surface where possible".to_string(),
        detail: "Useful for long-ride durability, fueling practice, and confidence at event scale."
            .to_string(),
    }
}

fn suggested_climbing_ride(goal: &XcEventGoal, in_taper_window: bool) -> XcSuggestedRideResponse {
    if in_taper_window {
        return suggested_taper_ride();
    }

    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::ClimbDurability,
        duration_seconds_min: Some(9_000),
        duration_seconds_max: Some(18_000),
        distance_meters_min: None,
        distance_meters_max: None,
        climbing_elevation_gain_meters: Some(
            (readiness_goal_elevation_gain_meters(goal) * 0.25).max(600.0),
        ),
        intensity: "Z2 on climbs with short tempo pressure only if fresh".to_string(),
        terrain: "Hilly MTB route or repeated climbs".to_string(),
        detail: "Useful for climbing durability and making the event elevation feel normal."
            .to_string(),
    }
}

fn suggested_specificity_ride(
    goal: &XcEventGoal,
    in_taper_window: bool,
) -> XcSuggestedRideResponse {
    if in_taper_window {
        return suggested_taper_ride();
    }

    let target_density = climb_density_meters_per_kilometer(
        Some(readiness_goal_elevation_gain_meters(goal)),
        Some(goal.target_distance_meters),
    );

    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::TechnicalFatigue,
        duration_seconds_min: Some(7_200),
        duration_seconds_max: Some(14_400),
        distance_meters_min: None,
        distance_meters_max: None,
        climbing_elevation_gain_meters: target_density.map(|density| density * 40.0),
        intensity: "Mostly aerobic with event-like surges".to_string(),
        terrain: terrain_focus_for_goal(goal),
        detail: "Useful for matching the event's climbing density and technical fatigue pattern."
            .to_string(),
    }
}

fn suggested_tempo_ride(goal: &XcEventGoal, in_taper_window: bool) -> XcSuggestedRideResponse {
    if in_taper_window {
        return suggested_taper_ride();
    }

    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::Tempo,
        duration_seconds_min: Some(5_400),
        duration_seconds_max: Some(9_000),
        distance_meters_min: Some((goal.target_distance_meters * 0.15).min(32_000.0)),
        distance_meters_max: Some((goal.target_distance_meters * 0.3).min(56_000.0)),
        climbing_elevation_gain_meters: None,
        intensity: "Z2 warmup, then 2-3 tempo blocks of 15-25 minutes".to_string(),
        terrain: "Controlled route where pacing stays steady".to_string(),
        detail:
            "Useful for lifting sustainable speed without turning the day into a maximal effort."
                .to_string(),
    }
}

fn suggested_durability_ride(goal: &XcEventGoal, in_taper_window: bool) -> XcSuggestedRideResponse {
    if in_taper_window {
        return suggested_taper_ride();
    }

    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::BaseEndurance,
        duration_seconds_min: Some(10_800),
        duration_seconds_max: Some(18_000),
        distance_meters_min: Some((goal.target_distance_meters * 0.25).min(48_000.0)),
        distance_meters_max: Some((goal.target_distance_meters * 0.45).min(80_000.0)),
        climbing_elevation_gain_meters: Some(readiness_goal_elevation_gain_meters(goal) * 0.2),
        intensity: "Strict Z2, steady fueling, no late hero pacing".to_string(),
        terrain: "Repeatable endurance route with reliable heart-rate data".to_string(),
        detail: "Useful for reducing decoupling and proving the pace survives the back half."
            .to_string(),
    }
}

fn suggested_recovery_ride() -> XcSuggestedRideResponse {
    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::Recovery,
        duration_seconds_min: Some(1_800),
        duration_seconds_max: Some(4_500),
        distance_meters_min: None,
        distance_meters_max: None,
        climbing_elevation_gain_meters: None,
        intensity: "Z1 to low Z2 only".to_string(),
        terrain: "Easy route with low consequence and low climbing pressure".to_string(),
        detail:
            "Useful for absorbing the current load before chasing another event-specific workout."
                .to_string(),
    }
}

fn suggested_taper_ride() -> XcSuggestedRideResponse {
    XcSuggestedRideResponse {
        purpose: XcTrainingPurpose::Recovery,
        duration_seconds_min: Some(2_700),
        duration_seconds_max: Some(5_400),
        distance_meters_min: None,
        distance_meters_max: None,
        climbing_elevation_gain_meters: None,
        intensity: "Easy endurance with a few short openers if fresh".to_string(),
        terrain: "Familiar route; avoid risky technical fatigue".to_string(),
        detail: "Useful for preserving freshness now that the event is close.".to_string(),
    }
}

fn terrain_focus_for_goal(goal: &XcEventGoal) -> String {
    match goal.event_profile {
        Some(XcEventProfile::TechnicalSingletrack) => {
            "Technical singletrack with punchy climbs and repeated accelerations".to_string()
        }
        Some(XcEventProfile::UltraMtb) => {
            "Long MTB route with self-supported pacing and fueling practice".to_string()
        }
        Some(XcEventProfile::XcMarathon) => {
            "Race-like MTB loop with sustained pedaling and controlled surges".to_string()
        }
        Some(XcEventProfile::EnduranceMtb) => {
            "Endurance MTB terrain with steady climbing and minimal stops".to_string()
        }
        Some(XcEventProfile::Custom) | None => {
            "Route that matches the target event's surface and climb density".to_string()
        }
    }
}

fn overall_readiness_status(gates: &[XcReadinessGateResponse]) -> XcReadinessStatus {
    if gates
        .iter()
        .any(|gate| gate.status == XcReadinessStatus::FallingBehind)
    {
        return XcReadinessStatus::FallingBehind;
    }
    if gates
        .iter()
        .any(|gate| gate.status == XcReadinessStatus::Watch)
    {
        return XcReadinessStatus::Watch;
    }
    let core_gates = gates
        .iter()
        .filter(|gate| gate.key != XcReadinessGateKey::Recovery)
        .collect::<Vec<_>>();

    if gates.is_empty()
        || core_gates
            .iter()
            .all(|gate| gate.status == XcReadinessStatus::MissingData)
    {
        return XcReadinessStatus::MissingData;
    }
    if core_gates
        .iter()
        .any(|gate| gate.status == XcReadinessStatus::MissingData)
    {
        return XcReadinessStatus::Watch;
    }

    XcReadinessStatus::OnTrack
}

fn readiness_reason(
    status: XcReadinessStatus,
    gates: &[XcReadinessGateResponse],
    deficits: &[XcTrainingDeficitResponse],
) -> String {
    if let Some(deficit) = deficits.first() {
        return match status {
            XcReadinessStatus::FallingBehind => {
                format!("{} is the biggest limiter right now.", deficit.title)
            }
            XcReadinessStatus::Watch => {
                format!(
                    "{} needs attention before it becomes the limiter.",
                    deficit.title
                )
            }
            _ => "The key event-specific gates are currently covered.".to_string(),
        };
    }

    if gates
        .iter()
        .any(|gate| gate.status == XcReadinessStatus::MissingData)
    {
        return "Some readiness signals are missing, but the available gates are not alarming."
            .to_string();
    }

    "The key event-specific gates are currently covered.".to_string()
}

fn apply_xc_training_purpose(ride: &mut XcRideProgressResponse, goal: Option<&XcEventGoal>) {
    let (purpose, detail) = classify_xc_training_purpose(ride, goal);
    ride.training_purpose = purpose;
    ride.training_purpose_detail = detail;
}

fn classify_xc_training_purpose(
    ride: &XcRideProgressResponse,
    goal: Option<&XcEventGoal>,
) -> (XcTrainingPurpose, String) {
    if ride.activity_type.is_race() {
        return (
            XcTrainingPurpose::DataQuality,
            "Race result is an outcome benchmark, not a training ride.".to_string(),
        );
    }

    let moving_time_seconds = ride.moving_time_seconds.unwrap_or_default();
    let z2_share = if moving_time_seconds > 0 {
        f64::from(ride.z2_time_seconds) / f64::from(moving_time_seconds)
    } else {
        0.0
    };
    let climbing_meters = effective_climbing_elevation_gain_meters(ride).unwrap_or_default();
    let ride_density =
        climb_density_meters_per_kilometer(Some(climbing_meters), ride.distance_meters);
    let target_density = goal.and_then(|goal| {
        climb_density_meters_per_kilometer(
            Some(readiness_goal_elevation_gain_meters(goal)),
            Some(goal.target_distance_meters),
        )
    });
    let high_density = match (ride_density, target_density) {
        (Some(ride_density), Some(target_density)) => ride_density >= target_density * 0.8,
        (Some(ride_density), None) => ride_density >= 25.0,
        _ => false,
    };

    if moving_time_seconds > 0 && moving_time_seconds <= 4_500 && climbing_meters < 250.0 {
        return (
            XcTrainingPurpose::Recovery,
            "Useful for recovery or aerobic maintenance without much event-specific load."
                .to_string(),
        );
    }
    if high_density || climbing_meters >= 750.0 {
        return (
            XcTrainingPurpose::ClimbDurability,
            "Useful for climbing durability and matching the event's elevation demand.".to_string(),
        );
    }
    if moving_time_seconds >= 10_800 && z2_share >= 0.4 {
        return (
            XcTrainingPurpose::BaseEndurance,
            "Useful for base endurance and long-ride durability.".to_string(),
        );
    }
    if ride
        .aerobic_decoupling_percent
        .is_some_and(|value| value > 6.0)
        && moving_time_seconds >= 7_200
    {
        return (
            XcTrainingPurpose::TechnicalFatigue,
            "Useful as a fatigue-resistance signal because durability faded late.".to_string(),
        );
    }
    if z2_share < 0.3 && moving_time_seconds >= 3_600 {
        return (
            XcTrainingPurpose::Tempo,
            "Useful for sustainable pressure, tempo, or mixed-intensity race pace.".to_string(),
        );
    }
    if ride.z2_time_seconds == 0 && climbing_meters <= 0.0 {
        return (
            XcTrainingPurpose::DataQuality,
            "Useful mostly as activity history; missing HR/elevation limits training interpretation.".to_string(),
        );
    }

    (
        XcTrainingPurpose::BaseEndurance,
        "Useful for aerobic consistency and filling the event training block.".to_string(),
    )
}

fn weighted_z2_speed_mps<'a>(
    rides: impl IntoIterator<Item = &'a XcRideProgressResponse>,
) -> Option<f64> {
    let mut distance_meters = 0.0;
    let mut time_seconds = 0;

    for ride in rides {
        if let Some(distance) = ride.z2_distance_meters {
            distance_meters += distance;
            time_seconds += ride.z2_time_seconds.max(0);
        }
    }

    (distance_meters > 0.0 && time_seconds > 0).then_some(distance_meters / f64::from(time_seconds))
}

fn target_finish_speed_mps(distance_meters: f64, target_finish_time_seconds: i32) -> Option<f64> {
    (distance_meters > 0.0 && target_finish_time_seconds > 0)
        .then_some(distance_meters / f64::from(target_finish_time_seconds))
}

fn readiness_goal_elevation_gain_meters(goal: &XcEventGoal) -> f64 {
    goal.target_elevation_gain_meters
        .min(XC_EVENT_TARGET_ELEVATION_GAIN_MAX_METERS)
}

fn big_climb_day_target_meters(goal: &XcEventGoal) -> f64 {
    readiness_goal_elevation_gain_meters(goal) * XC_BIG_CLIMB_DAY_TARGET_RATIO
}

fn recommendation_priority_rank(priority: TrainingRecommendationPriority) -> i32 {
    match priority {
        TrainingRecommendationPriority::High => 0,
        TrainingRecommendationPriority::Medium => 1,
        TrainingRecommendationPriority::Low => 2,
    }
}

fn deficit_rank(key: XcTrainingDeficitKey) -> i32 {
    match key {
        XcTrainingDeficitKey::Recovery => 0,
        XcTrainingDeficitKey::EventSpecificity => 1,
        XcTrainingDeficitKey::FinishPace => 2,
        XcTrainingDeficitKey::LongRide => 3,
        XcTrainingDeficitKey::BigClimbDay => 4,
        XcTrainingDeficitKey::AerobicDurability => 5,
    }
}

fn build_xc_race_results(rides: &[XcRideProgressResponse]) -> Vec<XcRaceResultResponse> {
    let mut races = rides
        .iter()
        .filter(|ride| ride.activity_type.is_race())
        .collect::<Vec<_>>();
    races.sort_by(|left, right| right.started_at.cmp(&left.started_at));

    races
        .into_iter()
        .take(5)
        .map(|race| {
            let prior_window_start = race.started_at - Duration::days(XC_RECENT_WINDOW_DAYS);
            let prior_training_rides = rides
                .iter()
                .filter(|ride| !ride.activity_type.is_race() && ride.started_at < race.started_at)
                .collect::<Vec<_>>();
            let prior_window_rides = prior_training_rides
                .iter()
                .copied()
                .filter(|ride| ride.started_at >= prior_window_start)
                .collect::<Vec<_>>();
            let prior_training_z2_time_seconds = prior_window_rides
                .iter()
                .map(|ride| ride.z2_time_seconds)
                .sum::<i32>();
            let prior_training_climbing_elevation_gain_meters = prior_window_rides
                .iter()
                .filter_map(|ride| effective_climbing_elevation_gain_meters(ride))
                .sum::<f64>();
            let prior_training_average_z2_speed_mps = average_f64(
                prior_window_rides
                    .iter()
                    .filter_map(|ride| ride.z2_average_speed_mps),
            );
            let prior_training_average_aerobic_decoupling_percent = average_f64(
                prior_window_rides
                    .iter()
                    .filter_map(|ride| ride.aerobic_decoupling_percent),
            );
            let best_training_distance_meters = prior_training_rides
                .iter()
                .filter_map(|ride| ride.distance_meters)
                .reduce(f64::max);
            let best_training_elevation_gain_meters = prior_training_rides
                .iter()
                .filter_map(|ride| effective_climbing_elevation_gain_meters(ride))
                .reduce(f64::max);
            let race_elevation_gain_meters = effective_climbing_elevation_gain_meters(race);
            let race_vs_best_training_distance_percent =
                relative_percent(race.distance_meters, best_training_distance_meters);
            let race_vs_best_training_elevation_percent = relative_percent(
                race_elevation_gain_meters,
                best_training_elevation_gain_meters,
            );
            let (insight_title, insight_detail) = race_insight(
                prior_window_rides.len(),
                prior_training_z2_time_seconds,
                prior_training_climbing_elevation_gain_meters,
                prior_training_average_aerobic_decoupling_percent,
                race_vs_best_training_distance_percent,
                race_vs_best_training_elevation_percent,
            );

            XcRaceResultResponse {
                activity_id: race.activity_id,
                activity_title: race.activity_title.clone(),
                started_at: race.started_at,
                distance_meters: race.distance_meters.map(round_metric),
                elevation_gain_meters: race_elevation_gain_meters.map(round_metric),
                moving_time_seconds: race.moving_time_seconds,
                average_speed_mps: average_speed_for_ride(race).map(round_metric),
                climb_density_meters_per_kilometer: climb_density_meters_per_kilometer(
                    race_elevation_gain_meters,
                    race.distance_meters,
                )
                .map(round_metric),
                z2_time_seconds: race.z2_time_seconds,
                climbing_time_seconds: race.climbing_time_seconds,
                climbing_elevation_gain_meters: race
                    .climbing_elevation_gain_meters
                    .map(round_metric),
                aerobic_decoupling_percent: race.aerobic_decoupling_percent.map(round_metric),
                prior_training_ride_count: prior_window_rides.len() as i32,
                prior_training_z2_time_seconds,
                prior_training_climbing_elevation_gain_meters: round_metric(
                    prior_training_climbing_elevation_gain_meters,
                ),
                prior_training_average_z2_speed_mps: prior_training_average_z2_speed_mps
                    .map(round_metric),
                prior_training_average_aerobic_decoupling_percent:
                    prior_training_average_aerobic_decoupling_percent.map(round_metric),
                race_vs_best_training_distance_percent: race_vs_best_training_distance_percent
                    .map(round_metric),
                race_vs_best_training_elevation_percent: race_vs_best_training_elevation_percent
                    .map(round_metric),
                insight_title,
                insight_detail,
            }
        })
        .collect()
}

fn race_insight(
    prior_training_ride_count: usize,
    prior_training_z2_time_seconds: i32,
    prior_training_climbing_elevation_gain_meters: f64,
    prior_training_average_aerobic_decoupling_percent: Option<f64>,
    race_vs_best_training_distance_percent: Option<f64>,
    race_vs_best_training_elevation_percent: Option<f64>,
) -> (String, String) {
    if prior_training_ride_count == 0 {
        return (
            "Race logged without a recent XC build".to_string(),
            "Mark earlier rides as XC or backfill the training block so Bike can compare this result against the work that led into it.".to_string(),
        );
    }

    if race_vs_best_training_distance_percent.is_some_and(|value| value >= 130.0)
        && f64::from(prior_training_z2_time_seconds) < XC_WEEKLY_Z2_GOAL_SECONDS * 3.0
    {
        return (
            "Race distance outpaced the recent endurance build".to_string(),
            "The result was much longer than your biggest prior training ride while recent Z2 volume was below the v1 build target. More long aerobic work should make the next race less costly.".to_string(),
        );
    }

    if race_vs_best_training_elevation_percent.is_some_and(|value| value >= 130.0)
        && prior_training_climbing_elevation_gain_meters < XC_WEEKLY_CLIMBING_GOAL_METERS * 3.0
    {
        return (
            "Climbing demand exceeded the recent build".to_string(),
            "The race packed more climbing than recent training prepared for. Add longer climbing-endurance days or hillier back-to-back rides before the next target.".to_string(),
        );
    }

    if prior_training_average_aerobic_decoupling_percent
        .is_some_and(|value| value <= XC_AEROBIC_DECOUPLING_GOAL_PERCENT)
    {
        return (
            "Durability work appears to be transferring".to_string(),
            "Recent comparable rides were inside the decoupling target before this race, which is a good sign that steady endurance work supported the result.".to_string(),
        );
    }

    (
        "Race result is ready for trend comparison".to_string(),
        "Keep importing race outcomes and comparable endurance rides so the XC page can separate training consistency from event-day demands.".to_string(),
    )
}

fn build_dh_goal_progress_response(
    segment_models: Vec<segments::Model>,
    efforts: Vec<DhEffortSource>,
    freshness: Option<FitnessFreshnessSnapshot>,
    now: DateTime<Utc>,
) -> DhGoalProgressResponse {
    let mut segments = build_dh_segment_progress(&segment_models, &efforts);
    segments.sort_by(|left, right| {
        right
            .latest_activity_started_at
            .cmp(&left.latest_activity_started_at)
            .then_with(|| left.segment_title.cmp(&right.segment_title))
    });
    let mut recent_sessions = build_dh_session_progress(&efforts);
    recent_sessions.sort_by(|left, right| right.started_at.cmp(&left.started_at));
    recent_sessions.truncate(DH_RECENT_SESSION_LIMIT);

    let session_count = recent_sessions.len() as i32;
    let effort_count = efforts.len() as i32;
    let average_efforts_per_session = if session_count > 0 {
        Some(f64::from(effort_count) / f64::from(session_count))
    } else {
        None
    };
    let average_repeat_fade_percent = average_f64(
        recent_sessions
            .iter()
            .filter_map(|session| session.average_repeat_fade_percent),
    );
    let average_top_3_gap_percent = average_f64(
        segments
            .iter()
            .filter_map(|segment| segment.top_3_pr_gap_percent),
    );

    let summary = DhProgressSummaryResponse {
        segment_count: segment_models.len() as i32,
        session_count,
        effort_count,
        average_efforts_per_session: average_efforts_per_session.map(round_metric),
        average_repeat_fade_percent: average_repeat_fade_percent.map(round_metric),
        average_top_3_gap_percent: average_top_3_gap_percent.map(round_metric),
    };
    let goals = vec![
        build_goal_metric(
            TrainingGoalKey::DhLapsPerSession,
            "DH laps per session",
            TrainingMetricUnit::Count,
            TrainingGoalDirection::AtLeast,
            average_efforts_per_session,
            DH_LAPS_PER_SESSION_GOAL,
        ),
        build_goal_metric(
            TrainingGoalKey::DhRepeatFade,
            "DH repeat fade",
            TrainingMetricUnit::Percent,
            TrainingGoalDirection::AtMost,
            average_repeat_fade_percent,
            DH_REPEAT_FADE_GOAL_PERCENT,
        ),
        build_goal_metric(
            TrainingGoalKey::DhRollingTop3Gap,
            "DH top-3 gap",
            TrainingMetricUnit::Percent,
            TrainingGoalDirection::AtMost,
            average_top_3_gap_percent,
            DH_TOP3_GAP_GOAL_PERCENT,
        ),
    ];
    let recommendations = build_dh_recommendations(
        segment_models.len(),
        session_count,
        average_efforts_per_session,
        average_repeat_fade_percent,
        average_top_3_gap_percent,
        freshness,
    );

    DhGoalProgressResponse {
        generated_at: now,
        summary,
        goals,
        recommendations,
        segments,
        recent_sessions,
    }
}

fn build_dh_segment_progress(
    segment_models: &[segments::Model],
    efforts: &[DhEffortSource],
) -> Vec<DhSegmentProgressResponse> {
    let mut efforts_by_segment_id = HashMap::<i32, Vec<DhEffortSource>>::new();
    for effort in efforts.iter().cloned() {
        efforts_by_segment_id
            .entry(effort.segment_id)
            .or_default()
            .push(effort);
    }

    segment_models
        .iter()
        .map(|segment| {
            let mut segment_efforts = efforts_by_segment_id
                .remove(&segment.id)
                .unwrap_or_default();
            segment_efforts.sort_by(|left, right| {
                right
                    .started_at
                    .cmp(&left.started_at)
                    .then_with(|| right.effort_index.cmp(&left.effort_index))
            });

            let personal_record_duration_seconds = segment_efforts
                .iter()
                .map(|effort| effort.duration_seconds)
                .min();
            let recent_best_duration_seconds = segment_efforts
                .iter()
                .take(DH_RECENT_EFFORT_LIMIT)
                .map(|effort| effort.duration_seconds)
                .min();
            let rolling_top_3_average_duration_seconds = if segment_efforts.len() >= 3 {
                Some(
                    segment_efforts
                        .iter()
                        .take(3)
                        .map(|effort| f64::from(effort.duration_seconds))
                        .sum::<f64>()
                        / 3.0,
                )
            } else {
                None
            };
            let top_3_pr_gap_percent = rolling_top_3_average_duration_seconds.and_then(|average| {
                personal_record_duration_seconds.and_then(|pr_seconds| {
                    let pr_seconds = f64::from(pr_seconds);
                    (pr_seconds > 0.0).then_some(((average - pr_seconds) / pr_seconds) * 100.0)
                })
            });
            let repeat_fade_percent = average_recent_segment_repeat_fade(&segment_efforts);
            let latest_effort = segment_efforts.first();

            DhSegmentProgressResponse {
                segment_id: segment.id,
                segment_title: segment.title.clone(),
                effort_count: segment_efforts.len() as i32,
                personal_record_duration_seconds,
                recent_best_duration_seconds,
                rolling_top_3_average_duration_seconds: rolling_top_3_average_duration_seconds
                    .map(round_metric),
                top_3_pr_gap_percent: top_3_pr_gap_percent.map(round_metric),
                repeat_fade_percent: repeat_fade_percent.map(round_metric),
                latest_activity_id: latest_effort.map(|effort| effort.activity_id),
                latest_activity_title: latest_effort.map(|effort| effort.activity_title.clone()),
                latest_activity_started_at: latest_effort.map(|effort| effort.started_at),
            }
        })
        .collect()
}

fn build_dh_session_progress(efforts: &[DhEffortSource]) -> Vec<DhSessionSummaryResponse> {
    let mut efforts_by_activity_id = BTreeMap::<i32, Vec<DhEffortSource>>::new();
    for effort in efforts.iter().cloned() {
        efforts_by_activity_id
            .entry(effort.activity_id)
            .or_default()
            .push(effort);
    }

    efforts_by_activity_id
        .into_values()
        .filter_map(|mut session_efforts| {
            session_efforts.sort_by(|left, right| left.effort_index.cmp(&right.effort_index));
            let first_effort = session_efforts.first()?;
            let mut efforts_by_segment_id = HashMap::<i32, Vec<DhEffortSource>>::new();
            for effort in session_efforts.iter().cloned() {
                efforts_by_segment_id
                    .entry(effort.segment_id)
                    .or_default()
                    .push(effort);
            }
            let average_repeat_fade_percent = average_f64(
                efforts_by_segment_id
                    .into_values()
                    .filter_map(|segment_efforts| session_repeat_fade_percent(&segment_efforts)),
            );

            Some(DhSessionSummaryResponse {
                activity_id: first_effort.activity_id,
                activity_title: first_effort.activity_title.clone(),
                started_at: first_effort.started_at,
                segment_count: session_efforts
                    .iter()
                    .map(|effort| effort.segment_id)
                    .collect::<std::collections::HashSet<_>>()
                    .len() as i32,
                effort_count: session_efforts.len() as i32,
                fastest_effort_duration_seconds: session_efforts
                    .iter()
                    .map(|effort| effort.duration_seconds)
                    .min(),
                average_repeat_fade_percent: average_repeat_fade_percent.map(round_metric),
            })
        })
        .collect()
}

fn average_recent_segment_repeat_fade(efforts: &[DhEffortSource]) -> Option<f64> {
    let mut efforts_by_activity_id = HashMap::<i32, Vec<DhEffortSource>>::new();
    for effort in efforts.iter().cloned() {
        efforts_by_activity_id
            .entry(effort.activity_id)
            .or_default()
            .push(effort);
    }

    let mut session_fades = efforts_by_activity_id
        .into_values()
        .filter_map(|segment_efforts| {
            let started_at = segment_efforts.first().map(|effort| effort.started_at)?;
            session_repeat_fade_percent(&segment_efforts).map(|fade| (started_at, fade))
        })
        .collect::<Vec<_>>();
    session_fades.sort_by(|left, right| right.0.cmp(&left.0));

    average_f64(
        session_fades
            .into_iter()
            .take(DH_RECENT_FADE_SESSION_LIMIT)
            .map(|(_, fade)| fade),
    )
}

fn session_repeat_fade_percent(efforts: &[DhEffortSource]) -> Option<f64> {
    if efforts.len() < 2 {
        return None;
    }

    let mut ordered_efforts = efforts.to_vec();
    ordered_efforts.sort_by(|left, right| left.effort_index.cmp(&right.effort_index));

    let first_duration_seconds = f64::from(ordered_efforts.first()?.duration_seconds);
    let last_duration_seconds = f64::from(ordered_efforts.last()?.duration_seconds);

    (first_duration_seconds > 0.0).then_some(
        ((last_duration_seconds - first_duration_seconds) / first_duration_seconds) * 100.0,
    )
}

fn build_xc_weekly_progress(
    rides: &[XcRideProgressResponse],
    now: DateTime<Utc>,
    goal: Option<&XcEventGoal>,
) -> Vec<XcWeeklyProgressPointResponse> {
    let history_end = goal.map_or(now.date_naive(), |goal| {
        std::cmp::min(now.date_naive(), goal.target_date)
    });
    let current_week_start = start_of_week(history_end);
    let first_week_start = goal.map_or_else(
        || current_week_start - Duration::days((XC_WEEKLY_PROGRESS_WEEKS - 1) * 7),
        |goal| start_of_week(goal.start_date),
    );
    let mut rides_by_week_start = BTreeMap::<NaiveDate, Vec<&XcRideProgressResponse>>::new();

    for ride in rides.iter().filter(|ride| {
        let ride_day = ride.started_at.date_naive();
        ride_day >= first_week_start && ride_day <= history_end
    }) {
        rides_by_week_start
            .entry(start_of_week(ride.started_at.date_naive()))
            .or_default()
            .push(ride);
    }

    let mut week_start = first_week_start;
    let mut points = Vec::new();
    while week_start <= current_week_start {
        let week_rides = rides_by_week_start.remove(&week_start).unwrap_or_default();
        let comparable_ride_count = week_rides
            .iter()
            .filter(|ride| ride.aerobic_decoupling_percent.is_some())
            .count() as i32;
        let z2_time_seconds = week_rides
            .iter()
            .map(|ride| ride.z2_time_seconds)
            .sum::<i32>();
        let distance_meters = week_rides
            .iter()
            .filter_map(|ride| ride.distance_meters)
            .sum::<f64>();
        let z2_distance_meters = week_rides
            .iter()
            .filter_map(|ride| ride.z2_distance_meters)
            .sum::<f64>();
        let climbing_time_seconds = week_rides
            .iter()
            .filter_map(|ride| effective_vertical_rate_time_seconds(ride))
            .sum::<i32>();
        let climbing_elevation_gain_meters = week_rides
            .iter()
            .filter_map(|ride| effective_climbing_elevation_gain_meters(ride))
            .sum::<f64>();

        points.push(XcWeeklyProgressPointResponse {
            week_start: week_start.format("%Y-%m-%d").to_string(),
            ride_count: week_rides.len() as i32,
            comparable_ride_count,
            distance_meters: round_metric(distance_meters),
            z2_time_seconds,
            z2_distance_meters: round_metric(z2_distance_meters),
            average_z2_speed_mps: (z2_time_seconds > 0 && z2_distance_meters > 0.0)
                .then_some(z2_distance_meters / f64::from(z2_time_seconds))
                .map(round_metric),
            climbing_time_seconds,
            climbing_elevation_gain_meters: round_metric(climbing_elevation_gain_meters),
            climbing_vertical_rate_meters_per_hour: (climbing_time_seconds > 0
                && climbing_elevation_gain_meters > 0.0)
                .then_some(
                    (climbing_elevation_gain_meters / f64::from(climbing_time_seconds)) * 3600.0,
                )
                .map(round_metric),
            average_aerobic_decoupling_percent: average_f64(
                week_rides
                    .iter()
                    .filter_map(|ride| ride.aerobic_decoupling_percent),
            )
            .map(round_metric),
            z1_seconds: week_rides.iter().map(|ride| ride.z1_seconds).sum(),
            z2_zone_seconds: week_rides.iter().map(|ride| ride.z2_zone_seconds).sum(),
            z3_seconds: week_rides.iter().map(|ride| ride.z3_seconds).sum(),
            z4_seconds: week_rides.iter().map(|ride| ride.z4_seconds).sum(),
            z5_seconds: week_rides.iter().map(|ride| ride.z5_seconds).sum(),
        });

        week_start += Duration::days(7);
    }

    points
}

fn effective_climbing_elevation_gain_meters(ride: &XcRideProgressResponse) -> Option<f64> {
    ride.climbing_elevation_gain_meters
        .or(ride.elevation_gain_meters)
}

fn effective_vertical_rate_time_seconds(ride: &XcRideProgressResponse) -> Option<i32> {
    if ride.climbing_time_seconds > 0 {
        return Some(ride.climbing_time_seconds);
    }

    ride.moving_time_seconds.filter(|seconds| *seconds > 0)
}

fn average_speed_for_ride(ride: &XcRideProgressResponse) -> Option<f64> {
    let distance_meters = ride.distance_meters?;
    let moving_time_seconds = ride.moving_time_seconds?;

    (distance_meters > 0.0 && moving_time_seconds > 0)
        .then_some(distance_meters / f64::from(moving_time_seconds))
}

fn climb_density_meters_per_kilometer(
    elevation_gain_meters: Option<f64>,
    distance_meters: Option<f64>,
) -> Option<f64> {
    let elevation_gain_meters = elevation_gain_meters?;
    let distance_meters = distance_meters?;

    (elevation_gain_meters > 0.0 && distance_meters > 0.0)
        .then_some(elevation_gain_meters / (distance_meters / 1000.0))
}

fn relative_percent(value: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    let value = value?;
    let baseline = baseline?;

    (value > 0.0 && baseline > 0.0).then_some((value / baseline) * 100.0)
}

fn build_xc_recommendations(
    rides: &[XcRideProgressResponse],
    recent_comparable_ride_count: usize,
    weekly_z2_average_seconds: f64,
    weekly_climbing_average_meters: f64,
    recent_decoupling_average: Option<f64>,
    freshness: Option<FitnessFreshnessSnapshot>,
) -> Vec<TrainingRecommendationResponse> {
    if rides.is_empty() {
        return vec![basic_training_recommendation(
            TrainingRecommendationKey::BuildXcBaseline,
            TrainingRecommendationPriority::High,
            "Build an XC baseline",
            "Complete a steady endurance ride with heart-rate data so the XC screen can start tracking durability and climbing work.",
        )];
    }

    let mut recommendations = Vec::new();
    let needs_recovery = freshness.is_some_and(freshness_needs_recovery);
    let ready_for_quality = freshness.is_some_and(|snapshot| {
        freshness_is_ready_for_quality(snapshot, XC_READY_FITNESS_THRESHOLD)
    });

    if needs_recovery {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::RecoverBeforeNextXcRide,
            TrainingRecommendationPriority::High,
            "Absorb the current XC load first",
            "Fatigue is outrunning fitness and form is deeply negative, so keep the next ride easy or shorter before adding more endurance volume or climbing demand.",
        ));
    }

    if recent_comparable_ride_count == 0 {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::RepeatComparableEnduranceRide,
            TrainingRecommendationPriority::High,
            "Repeat a comparable endurance route",
            "A steady XC endurance ride on a repeatable route will unlock stronger durability comparisons and decoupling trend lines.",
        ));
    }
    if !needs_recovery
        && recent_decoupling_average
            .is_some_and(|value| value > XC_AEROBIC_DECOUPLING_GOAL_PERCENT + 1.0)
    {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::HoldSteadyEndurance,
            TrainingRecommendationPriority::High,
            "Keep the next endurance ride steadier",
            "Decoupling is still elevated, so prioritize smoother pacing and fueling before pushing volume or intensity.",
        ));
    }
    if !needs_recovery && weekly_z2_average_seconds < XC_WEEKLY_Z2_GOAL_SECONDS {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::IncreaseEnduranceVolume,
            TrainingRecommendationPriority::Medium,
            "Add more weekly Z2 volume",
            "Your recent weekly Z2 average is under the v1 target, so another aerobic endurance ride would move the XC screen forward.",
        ));
    }
    if !needs_recovery && weekly_climbing_average_meters < XC_WEEKLY_CLIMBING_GOAL_METERS {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::AddClimbingEndurance,
            TrainingRecommendationPriority::Medium,
            "Add more climbing durability",
            "A longer climbing-focused endurance ride would improve the climbing side of the XC progression model.",
        ));
    }
    if recommendations.is_empty() {
        if ready_for_quality && recent_comparable_ride_count > 0 {
            recommendations.push(basic_training_recommendation(
                TrainingRecommendationKey::UsePositiveFormForXcBenchmark,
                TrainingRecommendationPriority::Low,
                "Use the good form for a benchmark XC ride",
                "Fitness is established, fatigue is under control, and form is positive, so this is a strong window for a longer comparable endurance or climbing benchmark ride.",
            ));
        } else {
            recommendations.push(basic_training_recommendation(
                TrainingRecommendationKey::MaintainEnduranceRhythm,
                TrainingRecommendationPriority::Low,
                "Maintain the current XC rhythm",
                "Your recent endurance volume and durability are tracking well, so keep stacking comparable rides for cleaner trend lines.",
            ));
        }
    }

    recommendations.truncate(3);
    recommendations
}

fn build_xc_event_recommendations(
    deficits: &[XcTrainingDeficitResponse],
    rides: &[XcRideProgressResponse],
    recent_comparable_ride_count: usize,
    weekly_z2_average_seconds: f64,
    weekly_climbing_average_meters: f64,
    recent_decoupling_average: Option<f64>,
    freshness: Option<FitnessFreshnessSnapshot>,
) -> Vec<TrainingRecommendationResponse> {
    if !deficits.is_empty() {
        return deficits
            .iter()
            .take(3)
            .map(recommendation_from_deficit)
            .collect();
    }

    build_xc_recommendations(
        rides,
        recent_comparable_ride_count,
        weekly_z2_average_seconds,
        weekly_climbing_average_meters,
        recent_decoupling_average,
        freshness,
    )
}

fn recommendation_from_deficit(
    deficit: &XcTrainingDeficitResponse,
) -> TrainingRecommendationResponse {
    TrainingRecommendationResponse {
        key: recommendation_key_for_deficit(deficit.key),
        priority: deficit.priority,
        title: next_ride_title_for_deficit(deficit.key).to_string(),
        detail: deficit.detail.clone(),
        purpose: Some(deficit.suggested_ride.purpose),
        limiter: Some(deficit.title.clone()),
        gap_value: deficit.gap_value,
        gap_unit: deficit.gap_unit,
        suggested_ride: Some(deficit.suggested_ride.clone()),
    }
}

fn recommendation_key_for_deficit(key: XcTrainingDeficitKey) -> TrainingRecommendationKey {
    match key {
        XcTrainingDeficitKey::Recovery => TrainingRecommendationKey::RecoverBeforeNextXcRide,
        XcTrainingDeficitKey::BigClimbDay | XcTrainingDeficitKey::EventSpecificity => {
            TrainingRecommendationKey::AddClimbingEndurance
        }
        XcTrainingDeficitKey::AerobicDurability => TrainingRecommendationKey::HoldSteadyEndurance,
        XcTrainingDeficitKey::FinishPace => {
            TrainingRecommendationKey::UsePositiveFormForXcBenchmark
        }
        XcTrainingDeficitKey::LongRide => TrainingRecommendationKey::IncreaseEnduranceVolume,
    }
}

fn next_ride_title_for_deficit(key: XcTrainingDeficitKey) -> &'static str {
    match key {
        XcTrainingDeficitKey::Recovery => "Make the next ride recovery-first",
        XcTrainingDeficitKey::BigClimbDay => "Schedule a bigger climbing day",
        XcTrainingDeficitKey::EventSpecificity => "Choose a more event-like route",
        XcTrainingDeficitKey::FinishPace => "Add controlled tempo pressure",
        XcTrainingDeficitKey::AerobicDurability => "Repeat a steadier endurance route",
        XcTrainingDeficitKey::LongRide => "Extend the next long ride",
    }
}

fn basic_training_recommendation(
    key: TrainingRecommendationKey,
    priority: TrainingRecommendationPriority,
    title: &str,
    detail: &str,
) -> TrainingRecommendationResponse {
    TrainingRecommendationResponse {
        key,
        priority,
        title: title.to_string(),
        detail: detail.to_string(),
        purpose: None,
        limiter: None,
        gap_value: None,
        gap_unit: None,
        suggested_ride: None,
    }
}

fn build_dh_recommendations(
    segment_count: usize,
    session_count: i32,
    average_efforts_per_session: Option<f64>,
    average_repeat_fade_percent: Option<f64>,
    average_top_3_gap_percent: Option<f64>,
    freshness: Option<FitnessFreshnessSnapshot>,
) -> Vec<TrainingRecommendationResponse> {
    if segment_count == 0 {
        return vec![basic_training_recommendation(
            TrainingRecommendationKey::MarkDhSegments,
            TrainingRecommendationPriority::High,
            "Mark a few segments as DH",
            "DH progress stays opt-in, so mark your core downhill laps first and the screen can start tracking PRs, top-3 averages, and repeat fade.",
        )];
    }
    if session_count == 0 {
        return vec![basic_training_recommendation(
            TrainingRecommendationKey::AddDhRepeats,
            TrainingRecommendationPriority::High,
            "Do repeat laps on a marked DH segment",
            "A few repeated downhill laps in the same session will unlock the first usable DH session and repeatability stats.",
        )];
    }

    let mut recommendations = Vec::new();
    let needs_recovery = freshness.is_some_and(freshness_needs_recovery);
    let ready_for_quality = freshness.is_some_and(|snapshot| {
        freshness_is_ready_for_quality(snapshot, DH_READY_FITNESS_THRESHOLD)
    });

    if needs_recovery {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::RecoverBeforeNextDhSession,
            TrainingRecommendationPriority::High,
            "Recover before the next DH session",
            "Fatigue is running ahead of fitness and form is negative, so keep the next downhill day shorter or technique-focused before adding more repeat laps.",
        ));
    }

    if !needs_recovery && average_efforts_per_session.unwrap_or_default() < DH_LAPS_PER_SESSION_GOAL
    {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::AddDhRepeats,
            TrainingRecommendationPriority::Medium,
            "Add more DH repeats per session",
            "The DH screen gets stronger with three or more laps per session because that makes repeat fade and rolling averages more meaningful.",
        ));
    }
    if !needs_recovery
        && average_repeat_fade_percent.unwrap_or_default() > DH_REPEAT_FADE_GOAL_PERCENT
    {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::ReduceDhFade,
            TrainingRecommendationPriority::High,
            "Reduce repeat fade",
            "Later laps are slowing down more than the v1 target, so recover a bit more between runs or cap session length before technique falls off.",
        ));
    }
    if !needs_recovery && average_top_3_gap_percent.unwrap_or_default() > DH_TOP3_GAP_GOAL_PERCENT {
        recommendations.push(basic_training_recommendation(
            TrainingRecommendationKey::ChaseDhConsistency,
            TrainingRecommendationPriority::Medium,
            "Bring the rolling top-3 closer to your PR",
            "Your recent downhill benchmark is still a few percent off the PR pace, so focus on consistent fast laps before chasing one-off hero runs.",
        ));
    }
    if recommendations.is_empty() {
        if ready_for_quality {
            recommendations.push(basic_training_recommendation(
                TrainingRecommendationKey::UsePositiveFormForDhBenchmark,
                TrainingRecommendationPriority::Low,
                "Use the positive form for a fast DH day",
                "Fitness is established, fatigue is under control, and form is positive, so this is a strong window for a sharper repeat-lap session on your marked DH segments.",
            ));
        } else {
            recommendations.push(basic_training_recommendation(
                TrainingRecommendationKey::MaintainDhMomentum,
                TrainingRecommendationPriority::Low,
                "Maintain downhill momentum",
                "Repeatability, recent top-3 pace, and lap count are all in a healthy place, so keep feeding the DH history with more marked sessions.",
            ));
        }
    }

    recommendations.truncate(3);
    recommendations
}

fn freshness_needs_recovery(snapshot: FitnessFreshnessSnapshot) -> bool {
    snapshot.form <= FRESHNESS_RECOVERY_FORM_THRESHOLD
        || snapshot.fatigue >= snapshot.fitness + FRESHNESS_RECOVERY_FATIGUE_MARGIN
}

fn freshness_is_ready_for_quality(
    snapshot: FitnessFreshnessSnapshot,
    minimum_fitness: f64,
) -> bool {
    snapshot.form >= FRESHNESS_READY_FORM_THRESHOLD
        && snapshot.fitness >= minimum_fitness
        && snapshot.fatigue <= snapshot.fitness + FRESHNESS_READY_FATIGUE_MARGIN
}

fn build_goal_metric(
    key: TrainingGoalKey,
    label: &str,
    unit: TrainingMetricUnit,
    direction: TrainingGoalDirection,
    current_value: Option<f64>,
    target_value: f64,
) -> TrainingGoalMetricResponse {
    TrainingGoalMetricResponse {
        key,
        label: label.to_string(),
        unit,
        direction,
        current_value: current_value.map(round_metric),
        target_value: round_metric(target_value),
        progress_percent: goal_progress_percent(current_value, target_value, direction)
            .map(round_metric),
    }
}

fn goal_progress_percent(
    current_value: Option<f64>,
    target_value: f64,
    direction: TrainingGoalDirection,
) -> Option<f64> {
    let current_value = current_value?;
    if target_value <= 0.0 {
        return None;
    }

    Some(match direction {
        TrainingGoalDirection::AtLeast => {
            ((current_value / target_value) * 100.0).clamp(0.0, 100.0)
        }
        TrainingGoalDirection::AtMost => {
            if current_value <= target_value {
                100.0
            } else {
                ((target_value / current_value) * 100.0).clamp(0.0, 100.0)
            }
        }
    })
}

fn average_f64(values: impl IntoIterator<Item = f64>) -> Option<f64> {
    let mut count = 0usize;
    let mut total = 0.0;

    for value in values {
        total += value;
        count += 1;
    }

    (count > 0).then_some(total / count as f64)
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn start_of_week(value: NaiveDate) -> NaiveDate {
    value - Duration::days(i64::from(value.weekday().num_days_from_monday()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_xc_ride(
        activity_id: i32,
        started_at: DateTime<Utc>,
        ride_focus: ActivityRideFocus,
        z2_time_seconds: i32,
        climbing_elevation_gain_meters: Option<f64>,
        aerobic_decoupling_percent: Option<f64>,
    ) -> XcRideProgressResponse {
        let z2_average_speed_mps = (z2_time_seconds > 0).then_some(3.2);
        XcRideProgressResponse {
            activity_id,
            activity_title: format!("Ride {activity_id}"),
            started_at,
            activity_type: ActivityType::Training,
            ride_focus,
            route_family_key: Some("post-canyon".to_string()),
            distance_meters: Some(32_000.0),
            elevation_gain_meters: Some(750.0),
            moving_time_seconds: Some(5_400),
            z2_time_seconds,
            z2_distance_meters: z2_average_speed_mps
                .map(|speed| speed * f64::from(z2_time_seconds)),
            z2_average_speed_mps,
            climbing_time_seconds: 1_200,
            climbing_elevation_gain_meters,
            aerobic_decoupling_percent,
            z1_seconds: 900,
            z2_zone_seconds: z2_time_seconds,
            z3_seconds: 300,
            z4_seconds: 0,
            z5_seconds: 0,
            training_purpose: XcTrainingPurpose::DataQuality,
            training_purpose_detail: "Test fixture".to_string(),
        }
    }

    fn build_dh_segment(id: i32, title: &str) -> segments::Model {
        let now = Utc::now();

        segments::Model {
            id,
            user_id: 1,
            title: title.to_string(),
            source: "manual".to_string(),
            mode: "dh".to_string(),
            starred: false,
            original_filename: None,
            format: Some("gpx".to_string()),
            distance_meters: Some(1_200.0),
            route_data_json: None,
            source_activity_id: None,
            source_start_route_point_index: None,
            source_end_route_point_index: None,
            last_activity_change_at: now,
            created_at: now,
            updated_at: now,
        }
    }

    fn build_dh_effort(
        segment_id: i32,
        activity_id: i32,
        started_at: DateTime<Utc>,
        effort_index: i32,
        duration_seconds: i32,
    ) -> DhEffortSource {
        DhEffortSource {
            segment_id,
            activity_id,
            activity_title: format!("DH Session {activity_id}"),
            started_at,
            effort_index,
            duration_seconds,
        }
    }

    #[test]
    fn xc_progress_uses_recent_window_and_weekly_buckets() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![
                build_xc_ride(
                    1,
                    chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    7_200,
                    Some(900.0),
                    Some(6.0),
                ),
                build_xc_ride(
                    2,
                    chrono::DateTime::parse_from_rfc3339("2026-05-10T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::MixedXc,
                    3_600,
                    Some(500.0),
                    None,
                ),
                build_xc_ride(
                    3,
                    chrono::DateTime::parse_from_rfc3339("2026-03-01T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    9_000,
                    Some(1_100.0),
                    Some(4.0),
                ),
            ],
            None,
            None,
            now,
        );

        assert_eq!(response.summary.recent_ride_count, 2);
        assert_eq!(response.summary.comparable_ride_count, 1);
        assert_eq!(response.summary.total_z2_time_seconds, 10_800);
        assert_eq!(
            response.weekly_progress.len(),
            XC_WEEKLY_PROGRESS_WEEKS as usize
        );
        let latest_week = response
            .weekly_progress
            .last()
            .expect("weekly point present");
        assert_eq!(latest_week.distance_meters, 32_000.0);
        assert_eq!(latest_week.z2_zone_seconds, 7_200);
        assert_eq!(latest_week.z3_seconds, 300);
        assert_eq!(latest_week.average_z2_speed_mps, Some(3.2));
        assert_eq!(
            latest_week.climbing_vertical_rate_meters_per_hour,
            Some(2700.0)
        );
        assert_eq!(response.goals[0].key, TrainingGoalKey::WeeklyZ2Average);
        assert_eq!(
            response.recommendations[0].key,
            TrainingRecommendationKey::IncreaseEnduranceVolume
        );
    }

    #[test]
    fn xc_progress_ignores_implausible_aerobic_decoupling_values() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-31T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![build_xc_ride(
                1,
                chrono::DateTime::parse_from_rfc3339("2026-07-31T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
                ActivityRideFocus::XcEndurance,
                3_600,
                Some(600.0),
                Some(-189.3),
            )],
            None,
            None,
            now,
        );

        assert_eq!(response.summary.comparable_ride_count, 0);
        assert_eq!(response.summary.average_aerobic_decoupling_percent, None);
        assert_eq!(
            response
                .recent_rides
                .first()
                .and_then(|ride| ride.aerobic_decoupling_percent),
            None
        );
        assert_eq!(
            response
                .weekly_progress
                .last()
                .and_then(|point| point.average_aerobic_decoupling_percent),
            None
        );
    }

    #[test]
    fn xc_weekly_progress_keeps_positive_and_negative_decoupling_averages() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-08-16T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![
                build_xc_ride(
                    1,
                    chrono::DateTime::parse_from_rfc3339("2026-08-04T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    3_600,
                    Some(600.0),
                    Some(8.0),
                ),
                build_xc_ride(
                    2,
                    chrono::DateTime::parse_from_rfc3339("2026-08-05T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    3_600,
                    Some(600.0),
                    Some(10.0),
                ),
                build_xc_ride(
                    3,
                    chrono::DateTime::parse_from_rfc3339("2026-08-11T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    3_600,
                    Some(600.0),
                    Some(-12.0),
                ),
                build_xc_ride(
                    4,
                    chrono::DateTime::parse_from_rfc3339("2026-08-12T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    3_600,
                    Some(600.0),
                    Some(4.0),
                ),
            ],
            None,
            None,
            now,
        );

        let positive_week = response
            .weekly_progress
            .iter()
            .find(|point| point.week_start == "2026-08-03")
            .expect("positive decoupling week present");
        let negative_week = response
            .weekly_progress
            .iter()
            .find(|point| point.week_start == "2026-08-10")
            .expect("negative decoupling week present");

        assert_eq!(positive_week.comparable_ride_count, 2);
        assert_eq!(positive_week.average_aerobic_decoupling_percent, Some(9.0));
        assert_eq!(negative_week.comparable_ride_count, 2);
        assert_eq!(negative_week.average_aerobic_decoupling_percent, Some(-4.0));
    }

    #[test]
    fn xc_recommendations_request_comparable_ride_when_missing() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![build_xc_ride(
                1,
                chrono::DateTime::parse_from_rfc3339("2026-05-20T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
                ActivityRideFocus::MixedXc,
                4_200,
                Some(650.0),
                None,
            )],
            None,
            None,
            now,
        );

        assert_eq!(
            response.recommendations[0].key,
            TrainingRecommendationKey::RepeatComparableEnduranceRide
        );
    }

    #[test]
    fn xc_event_goal_counts_only_rides_within_saved_training_window() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![
                build_xc_ride(
                    1,
                    chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    7_200,
                    Some(2_700.0),
                    Some(4.3),
                ),
                XcRideProgressResponse {
                    distance_meters: Some(120_000.0),
                    elevation_gain_meters: Some(1_900.0),
                    ..build_xc_ride(
                        2,
                        chrono::DateTime::parse_from_rfc3339("2026-05-10T12:00:00Z")
                            .unwrap()
                            .with_timezone(&Utc),
                        ActivityRideFocus::XcEndurance,
                        8_400,
                        Some(1_900.0),
                        Some(5.0),
                    )
                },
                XcRideProgressResponse {
                    distance_meters: Some(200_000.0),
                    elevation_gain_meters: Some(5_000.0),
                    ..build_xc_ride(
                        3,
                        chrono::DateTime::parse_from_rfc3339("2026-03-20T12:00:00Z")
                            .unwrap()
                            .with_timezone(&Utc),
                        ActivityRideFocus::XcEndurance,
                        9_000,
                        Some(5_000.0),
                        Some(3.8),
                    )
                },
            ],
            Some(XcEventGoal {
                start_date: NaiveDate::from_ymd_opt(2026, 4, 1).unwrap(),
                target_date: NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
                target_distance_meters: 160_934.4,
                target_elevation_gain_meters: 3_962.4,
                event_name: Some("Marji Gesick".to_string()),
                target_finish_time_seconds: Some(43_200),
                event_profile: Some(XcEventProfile::TechnicalSingletrack),
            }),
            None,
            now,
        );

        let readiness = response.readiness.as_ref().expect("readiness present");
        let gate_keys = readiness
            .gates
            .iter()
            .map(|gate| gate.key)
            .collect::<Vec<_>>();
        assert!(gate_keys.contains(&XcReadinessGateKey::LongRideDistance));
        assert!(gate_keys.contains(&XcReadinessGateKey::BigClimbDay));

        let event_goal = response.event_goal.expect("event goal present");
        assert_eq!(event_goal.start_date, "2026-04-01");
        assert_eq!(event_goal.target_date, "2026-09-20");
        assert_eq!(event_goal.days_remaining, 122);
        assert_eq!(event_goal.training_window_days, 173);
        assert_eq!(event_goal.counted_ride_count, 2);
        assert_eq!(event_goal.counted_distance_meters, 152_000.0);
        assert_eq!(event_goal.counted_elevation_gain_meters, 4_600.0);
    }

    #[test]
    fn xc_event_goal_expands_visible_history_to_the_saved_training_block() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![
                build_xc_ride(
                    1,
                    chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    7_200,
                    Some(900.0),
                    Some(4.2),
                ),
                build_xc_ride(
                    2,
                    chrono::DateTime::parse_from_rfc3339("2026-04-10T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::MixedXc,
                    6_000,
                    Some(700.0),
                    None,
                ),
                build_xc_ride(
                    3,
                    chrono::DateTime::parse_from_rfc3339("2026-02-20T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    8_100,
                    Some(1_100.0),
                    Some(4.8),
                ),
                build_xc_ride(
                    4,
                    chrono::DateTime::parse_from_rfc3339("2026-01-10T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    9_000,
                    Some(1_250.0),
                    Some(5.1),
                ),
            ],
            Some(XcEventGoal {
                start_date: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                target_date: NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
                target_distance_meters: 160_934.4,
                target_elevation_gain_meters: 3_962.4,
                event_name: Some("Marji Gesick".to_string()),
                target_finish_time_seconds: Some(43_200),
                event_profile: Some(XcEventProfile::TechnicalSingletrack),
            }),
            None,
            now,
        );

        assert_eq!(
            response
                .recent_rides
                .iter()
                .map(|ride| ride.activity_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(
            response
                .weekly_progress
                .first()
                .map(|point| point.week_start.as_str()),
            Some("2026-01-26")
        );
        assert!(response.weekly_progress.len() > XC_WEEKLY_PROGRESS_WEEKS as usize);
    }

    #[test]
    fn xc_readiness_caps_typoed_climbing_target_for_benchmarks() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_xc_goal_progress_response(
            vec![XcRideProgressResponse {
                distance_meters: Some(64_000.0),
                elevation_gain_meters: Some(1_945.0),
                ..build_xc_ride(
                    1,
                    chrono::DateTime::parse_from_rfc3339("2026-05-18T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                    ActivityRideFocus::XcEndurance,
                    7_200,
                    Some(1_945.0),
                    Some(4.2),
                )
            }],
            Some(XcEventGoal {
                start_date: NaiveDate::from_ymd_opt(2026, 3, 1).unwrap(),
                target_date: NaiveDate::from_ymd_opt(2026, 9, 20).unwrap(),
                target_distance_meters: 110.0 * 1609.344,
                target_elevation_gain_meters: 130_000.0 * 0.3048,
                event_name: Some("Typoed climbing target".to_string()),
                target_finish_time_seconds: Some(41_250),
                event_profile: Some(XcEventProfile::UltraMtb),
            }),
            None,
            now,
        );

        let readiness = response.readiness.as_ref().expect("readiness present");
        let climb_day = readiness
            .gates
            .iter()
            .find(|gate| gate.key == XcReadinessGateKey::BigClimbDay)
            .expect("climb day gate present");
        assert_eq!(
            climb_day.target_value,
            Some(round_metric(
                XC_EVENT_TARGET_ELEVATION_GAIN_MAX_METERS * XC_BIG_CLIMB_DAY_TARGET_RATIO
            ))
        );

        let climb_density = readiness
            .gates
            .iter()
            .find(|gate| gate.key == XcReadinessGateKey::ClimbDensity)
            .expect("climb density gate present");
        assert_eq!(climb_density.target_value, Some(34.4));

        let climb_deficit = response
            .deficits
            .iter()
            .find(|deficit| deficit.key == XcTrainingDeficitKey::BigClimbDay)
            .expect("big climb deficit present");
        assert_eq!(
            climb_deficit.suggested_ride.climbing_elevation_gain_meters,
            Some(XC_EVENT_TARGET_ELEVATION_GAIN_MAX_METERS * 0.25)
        );
    }

    #[test]
    fn dh_segment_progress_computes_pr_top_3_and_repeat_fade() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let segments = vec![build_dh_segment(10, "FMR")];
        let efforts = vec![
            build_dh_effort(10, 100, now - Duration::days(1), 1, 300),
            build_dh_effort(10, 100, now - Duration::days(1), 2, 312),
            build_dh_effort(10, 101, now - Duration::days(4), 1, 298),
            build_dh_effort(10, 101, now - Duration::days(4), 2, 305),
            build_dh_effort(10, 102, now - Duration::days(8), 1, 296),
        ];

        let progress = build_dh_segment_progress(&segments, &efforts);
        let segment = &progress[0];

        assert_eq!(segment.personal_record_duration_seconds, Some(296));
        assert_eq!(segment.recent_best_duration_seconds, Some(296));
        assert_eq!(segment.rolling_top_3_average_duration_seconds, Some(305.7));
        assert_eq!(segment.top_3_pr_gap_percent, Some(3.3));
        assert_eq!(segment.repeat_fade_percent, Some(3.2));
    }

    #[test]
    fn dh_progress_recommends_marking_segments_when_none_exist() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-05-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let response = build_dh_goal_progress_response(Vec::new(), Vec::new(), None, now);

        assert_eq!(response.summary.segment_count, 0);
        assert_eq!(response.recommendations.len(), 1);
        assert_eq!(
            response.recommendations[0].key,
            TrainingRecommendationKey::MarkDhSegments
        );
    }

    #[test]
    fn xc_recommendations_prioritize_recovery_when_fatigue_is_high() {
        let recommendations = build_xc_recommendations(
            &[build_xc_ride(
                1,
                chrono::DateTime::parse_from_rfc3339("2026-05-20T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
                ActivityRideFocus::XcEndurance,
                7_200,
                Some(900.0),
                Some(4.2),
            )],
            1,
            XC_WEEKLY_Z2_GOAL_SECONDS,
            XC_WEEKLY_CLIMBING_GOAL_METERS,
            Some(4.2),
            Some(FitnessFreshnessSnapshot {
                day: NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
                fitness: 32.0,
                fatigue: 46.0,
                form: -14.0,
            }),
        );

        assert_eq!(
            recommendations[0].key,
            TrainingRecommendationKey::RecoverBeforeNextXcRide
        );
    }

    #[test]
    fn xc_recommendations_use_positive_form_when_metrics_are_stable() {
        let recommendations = build_xc_recommendations(
            &[build_xc_ride(
                1,
                chrono::DateTime::parse_from_rfc3339("2026-05-20T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
                ActivityRideFocus::XcEndurance,
                7_200,
                Some(900.0),
                Some(4.2),
            )],
            1,
            XC_WEEKLY_Z2_GOAL_SECONDS,
            XC_WEEKLY_CLIMBING_GOAL_METERS,
            Some(4.2),
            Some(FitnessFreshnessSnapshot {
                day: NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
                fitness: 34.0,
                fatigue: 31.0,
                form: 7.0,
            }),
        );

        assert_eq!(
            recommendations[0].key,
            TrainingRecommendationKey::UsePositiveFormForXcBenchmark
        );
    }

    #[test]
    fn dh_recommendations_prioritize_recovery_when_fatigue_is_high() {
        let recommendations = build_dh_recommendations(
            2,
            3,
            Some(3.0),
            Some(4.0),
            Some(2.5),
            Some(FitnessFreshnessSnapshot {
                day: NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
                fitness: 18.0,
                fatigue: 28.0,
                form: -10.0,
            }),
        );

        assert_eq!(
            recommendations[0].key,
            TrainingRecommendationKey::RecoverBeforeNextDhSession
        );
    }

    #[test]
    fn dh_recommendations_use_positive_form_when_metrics_are_stable() {
        let recommendations = build_dh_recommendations(
            2,
            3,
            Some(3.2),
            Some(4.0),
            Some(2.5),
            Some(FitnessFreshnessSnapshot {
                day: NaiveDate::from_ymd_opt(2026, 5, 21).unwrap(),
                fitness: 16.0,
                fatigue: 14.0,
                form: 6.0,
            }),
        );

        assert_eq!(
            recommendations[0].key,
            TrainingRecommendationKey::UsePositiveFormForDhBenchmark
        );
    }
}
