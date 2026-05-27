use crate::activity_training_analysis::ActivityRideFocus;
use crate::analytics::{FATIGUE_WINDOW_DAYS, FITNESS_WINDOW_DAYS};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{
    activities, activity_training_analyses, analytics_user_states, fitness_freshness_daily,
    segment_efforts, segments, user_preferences,
};
use crate::storage::AppStorage;
use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Datelike, Duration, NaiveDate, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use utoipa::ToSchema;

const XC_RECENT_WINDOW_DAYS: i64 = 28;
const XC_DECOUPLING_WINDOW_DAYS: i64 = 90;
const XC_WEEKLY_PROGRESS_WEEKS: i64 = 8;
const XC_RECENT_RIDES_LIMIT: usize = 12;
const XC_WEEKLY_Z2_GOAL_SECONDS: f64 = 14_400.0;
const XC_WEEKLY_CLIMBING_GOAL_METERS: f64 = 1_500.0;
const XC_AEROBIC_DECOUPLING_GOAL_PERCENT: f64 = 5.0;

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
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcWeeklyProgressPointResponse {
    pub week_start: String,
    pub ride_count: i32,
    pub comparable_ride_count: i32,
    pub z2_time_seconds: i32,
    pub z2_distance_meters: f64,
    pub average_z2_speed_mps: Option<f64>,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: f64,
    pub climbing_vertical_rate_meters_per_hour: Option<f64>,
    pub average_aerobic_decoupling_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcGoalProgressResponse {
    pub generated_at: DateTime<Utc>,
    pub event_goal: Option<XcEventGoalResponse>,
    pub summary: XcProgressSummaryResponse,
    pub goals: Vec<TrainingGoalMetricResponse>,
    pub recommendations: Vec<TrainingRecommendationResponse>,
    pub weekly_progress: Vec<XcWeeklyProgressPointResponse>,
    pub recent_rides: Vec<XcRideProgressResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcEventGoalResponse {
    pub start_date: String,
    pub target_date: String,
    pub days_remaining: i64,
    pub target_distance_meters: f64,
    pub target_elevation_gain_meters: f64,
    pub training_window_days: i32,
    pub counted_ride_count: i32,
    pub counted_distance_meters: f64,
    pub counted_distance_progress_percent: f64,
    pub counted_elevation_gain_meters: f64,
    pub counted_elevation_gain_progress_percent: f64,
    pub best_distance_meters: Option<f64>,
    pub best_distance_progress_percent: Option<f64>,
    pub best_distance_activity: Option<XcGoalActivityReferenceResponse>,
    pub best_elevation_gain_meters: Option<f64>,
    pub best_elevation_gain_progress_percent: Option<f64>,
    pub best_elevation_activity: Option<XcGoalActivityReferenceResponse>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct XcGoalActivityReferenceResponse {
    pub activity_id: i32,
    pub activity_title: String,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy)]
struct XcEventGoal {
    start_date: NaiveDate,
    target_date: NaiveDate,
    target_distance_meters: f64,
    target_elevation_gain_meters: f64,
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
        .filter(activity_training_analyses::Column::RideFocus.is_in(["xc_endurance", "mixed_xc"]))
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

            Some(XcRideProgressResponse {
                activity_id: activity.id,
                activity_title: activity.title.clone(),
                started_at: activity.started_at,
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
                aerobic_decoupling_percent: analysis.aerobic_decoupling_percent,
            })
        })
        .collect::<Vec<_>>();

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
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .into_model::<ActivitySummaryRow>()
        .all(db)
        .await?
        .into_iter()
        .map(|activity| (activity.id, activity))
        .collect())
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

    let event_goal = goal.and_then(|goal| build_xc_event_goal_response(&rides, goal, now));
    let history_end = goal.map(|goal| std::cmp::min(now.date_naive(), goal.target_date));

    let recent_window_start = now - Duration::days(XC_RECENT_WINDOW_DAYS);
    let decoupling_window_start = now - Duration::days(XC_DECOUPLING_WINDOW_DAYS);
    let recent_rides = rides
        .iter()
        .filter(|ride| ride.started_at >= recent_window_start)
        .collect::<Vec<_>>();
    let recent_comparable_rides = recent_rides
        .iter()
        .filter(|ride| ride.aerobic_decoupling_percent.is_some())
        .copied()
        .collect::<Vec<_>>();
    let recent_decoupling_average = average_f64(
        rides
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
    let recommendations = build_xc_recommendations(
        &rides,
        recent_comparable_rides.len(),
        weekly_z2_average_seconds,
        weekly_climbing_average_meters,
        recent_decoupling_average,
        freshness,
    );
    let weekly_progress = build_xc_weekly_progress(&rides, now, goal);
    let recent_rides = rides
        .into_iter()
        .filter(|ride| match (goal, history_end) {
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
        summary,
        goals,
        recommendations,
        weekly_progress,
        recent_rides,
    }
}

fn build_xc_event_goal_response(
    rides: &[XcRideProgressResponse],
    goal: XcEventGoal,
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
    let best_distance_ride = counted_rides.iter().copied().max_by(|left, right| {
        left.distance_meters
            .unwrap_or_default()
            .partial_cmp(&right.distance_meters.unwrap_or_default())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best_elevation_ride = counted_rides.iter().copied().max_by(|left, right| {
        left.climbing_elevation_gain_meters
            .or(left.elevation_gain_meters)
            .unwrap_or_default()
            .partial_cmp(
                &right
                    .climbing_elevation_gain_meters
                    .or(right.elevation_gain_meters)
                    .unwrap_or_default(),
            )
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let best_distance_meters = best_distance_ride.and_then(|ride| ride.distance_meters);
    let best_elevation_gain_meters = best_elevation_ride.and_then(|ride| {
        ride.climbing_elevation_gain_meters
            .or(ride.elevation_gain_meters)
    });

    Some(XcEventGoalResponse {
        start_date: goal.start_date.format("%Y-%m-%d").to_string(),
        target_date: goal.target_date.format("%Y-%m-%d").to_string(),
        days_remaining: goal
            .target_date
            .signed_duration_since(now.date_naive())
            .num_days(),
        target_distance_meters: round_metric(goal.target_distance_meters),
        target_elevation_gain_meters: round_metric(goal.target_elevation_gain_meters),
        training_window_days: (goal
            .target_date
            .signed_duration_since(goal.start_date)
            .num_days()
            + 1) as i32,
        counted_ride_count: counted_rides.len() as i32,
        counted_distance_meters: round_metric(counted_distance_meters),
        counted_distance_progress_percent: round_metric(
            goal_progress_percent(
                Some(counted_distance_meters),
                goal.target_distance_meters,
                TrainingGoalDirection::AtLeast,
            )
            .unwrap_or_default(),
        ),
        counted_elevation_gain_meters: round_metric(counted_elevation_gain_meters),
        counted_elevation_gain_progress_percent: round_metric(
            goal_progress_percent(
                Some(counted_elevation_gain_meters),
                goal.target_elevation_gain_meters,
                TrainingGoalDirection::AtLeast,
            )
            .unwrap_or_default(),
        ),
        best_distance_meters: best_distance_meters.map(round_metric),
        best_distance_progress_percent: goal_progress_percent(
            best_distance_meters,
            goal.target_distance_meters,
            TrainingGoalDirection::AtLeast,
        )
        .map(round_metric),
        best_distance_activity: best_distance_ride.map(goal_activity_reference),
        best_elevation_gain_meters: best_elevation_gain_meters.map(round_metric),
        best_elevation_gain_progress_percent: goal_progress_percent(
            best_elevation_gain_meters,
            goal.target_elevation_gain_meters,
            TrainingGoalDirection::AtLeast,
        )
        .map(round_metric),
        best_elevation_activity: best_elevation_ride.map(goal_activity_reference),
    })
}

fn goal_activity_reference(ride: &XcRideProgressResponse) -> XcGoalActivityReferenceResponse {
    XcGoalActivityReferenceResponse {
        activity_id: ride.activity_id,
        activity_title: ride.activity_title.clone(),
        started_at: ride.started_at,
    }
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
    goal: Option<XcEventGoal>,
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

fn build_xc_recommendations(
    rides: &[XcRideProgressResponse],
    recent_comparable_ride_count: usize,
    weekly_z2_average_seconds: f64,
    weekly_climbing_average_meters: f64,
    recent_decoupling_average: Option<f64>,
    freshness: Option<FitnessFreshnessSnapshot>,
) -> Vec<TrainingRecommendationResponse> {
    if rides.is_empty() {
        return vec![TrainingRecommendationResponse {
            key: TrainingRecommendationKey::BuildXcBaseline,
            priority: TrainingRecommendationPriority::High,
            title: "Build an XC baseline".to_string(),
            detail: "Complete a steady endurance ride with heart-rate data so the XC screen can start tracking durability and climbing work.".to_string(),
        }];
    }

    let mut recommendations = Vec::new();
    let needs_recovery = freshness.is_some_and(freshness_needs_recovery);
    let ready_for_quality = freshness.is_some_and(|snapshot| {
        freshness_is_ready_for_quality(snapshot, XC_READY_FITNESS_THRESHOLD)
    });

    if needs_recovery {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::RecoverBeforeNextXcRide,
            priority: TrainingRecommendationPriority::High,
            title: "Absorb the current XC load first".to_string(),
            detail: "Fatigue is outrunning fitness and form is deeply negative, so keep the next ride easy or shorter before adding more endurance volume or climbing demand.".to_string(),
        });
    }

    if recent_comparable_ride_count == 0 {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::RepeatComparableEnduranceRide,
            priority: TrainingRecommendationPriority::High,
            title: "Repeat a comparable endurance route".to_string(),
            detail: "A steady XC endurance ride on a repeatable route will unlock stronger durability comparisons and decoupling trend lines.".to_string(),
        });
    }
    if !needs_recovery
        && recent_decoupling_average
            .is_some_and(|value| value > XC_AEROBIC_DECOUPLING_GOAL_PERCENT + 1.0)
    {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::HoldSteadyEndurance,
            priority: TrainingRecommendationPriority::High,
            title: "Keep the next endurance ride steadier".to_string(),
            detail: "Decoupling is still elevated, so prioritize smoother pacing and fueling before pushing volume or intensity.".to_string(),
        });
    }
    if !needs_recovery && weekly_z2_average_seconds < XC_WEEKLY_Z2_GOAL_SECONDS {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::IncreaseEnduranceVolume,
            priority: TrainingRecommendationPriority::Medium,
            title: "Add more weekly Z2 volume".to_string(),
            detail: "Your recent weekly Z2 average is under the v1 target, so another aerobic endurance ride would move the XC screen forward.".to_string(),
        });
    }
    if !needs_recovery && weekly_climbing_average_meters < XC_WEEKLY_CLIMBING_GOAL_METERS {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::AddClimbingEndurance,
            priority: TrainingRecommendationPriority::Medium,
            title: "Add more climbing durability".to_string(),
            detail: "A longer climbing-focused endurance ride would improve the climbing side of the XC progression model.".to_string(),
        });
    }
    if recommendations.is_empty() {
        if ready_for_quality && recent_comparable_ride_count > 0 {
            recommendations.push(TrainingRecommendationResponse {
                key: TrainingRecommendationKey::UsePositiveFormForXcBenchmark,
                priority: TrainingRecommendationPriority::Low,
                title: "Use the good form for a benchmark XC ride".to_string(),
                detail: "Fitness is established, fatigue is under control, and form is positive, so this is a strong window for a longer comparable endurance or climbing benchmark ride.".to_string(),
            });
        } else {
            recommendations.push(TrainingRecommendationResponse {
                key: TrainingRecommendationKey::MaintainEnduranceRhythm,
                priority: TrainingRecommendationPriority::Low,
                title: "Maintain the current XC rhythm".to_string(),
                detail: "Your recent endurance volume and durability are tracking well, so keep stacking comparable rides for cleaner trend lines.".to_string(),
            });
        }
    }

    recommendations.truncate(3);
    recommendations
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
        return vec![TrainingRecommendationResponse {
            key: TrainingRecommendationKey::MarkDhSegments,
            priority: TrainingRecommendationPriority::High,
            title: "Mark a few segments as DH".to_string(),
            detail: "DH progress stays opt-in, so mark your core downhill laps first and the screen can start tracking PRs, top-3 averages, and repeat fade.".to_string(),
        }];
    }
    if session_count == 0 {
        return vec![TrainingRecommendationResponse {
            key: TrainingRecommendationKey::AddDhRepeats,
            priority: TrainingRecommendationPriority::High,
            title: "Do repeat laps on a marked DH segment".to_string(),
            detail: "A few repeated downhill laps in the same session will unlock the first usable DH session and repeatability stats.".to_string(),
        }];
    }

    let mut recommendations = Vec::new();
    let needs_recovery = freshness.is_some_and(freshness_needs_recovery);
    let ready_for_quality = freshness.is_some_and(|snapshot| {
        freshness_is_ready_for_quality(snapshot, DH_READY_FITNESS_THRESHOLD)
    });

    if needs_recovery {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::RecoverBeforeNextDhSession,
            priority: TrainingRecommendationPriority::High,
            title: "Recover before the next DH session".to_string(),
            detail: "Fatigue is running ahead of fitness and form is negative, so keep the next downhill day shorter or technique-focused before adding more repeat laps.".to_string(),
        });
    }

    if !needs_recovery && average_efforts_per_session.unwrap_or_default() < DH_LAPS_PER_SESSION_GOAL
    {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::AddDhRepeats,
            priority: TrainingRecommendationPriority::Medium,
            title: "Add more DH repeats per session".to_string(),
            detail: "The DH screen gets stronger with three or more laps per session because that makes repeat fade and rolling averages more meaningful.".to_string(),
        });
    }
    if !needs_recovery
        && average_repeat_fade_percent.unwrap_or_default() > DH_REPEAT_FADE_GOAL_PERCENT
    {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::ReduceDhFade,
            priority: TrainingRecommendationPriority::High,
            title: "Reduce repeat fade".to_string(),
            detail: "Later laps are slowing down more than the v1 target, so recover a bit more between runs or cap session length before technique falls off.".to_string(),
        });
    }
    if !needs_recovery && average_top_3_gap_percent.unwrap_or_default() > DH_TOP3_GAP_GOAL_PERCENT {
        recommendations.push(TrainingRecommendationResponse {
            key: TrainingRecommendationKey::ChaseDhConsistency,
            priority: TrainingRecommendationPriority::Medium,
            title: "Bring the rolling top-3 closer to your PR".to_string(),
            detail: "Your recent downhill benchmark is still a few percent off the PR pace, so focus on consistent fast laps before chasing one-off hero runs.".to_string(),
        });
    }
    if recommendations.is_empty() {
        if ready_for_quality {
            recommendations.push(TrainingRecommendationResponse {
                key: TrainingRecommendationKey::UsePositiveFormForDhBenchmark,
                priority: TrainingRecommendationPriority::Low,
                title: "Use the positive form for a fast DH day".to_string(),
                detail: "Fitness is established, fatigue is under control, and form is positive, so this is a strong window for a sharper repeat-lap session on your marked DH segments.".to_string(),
            });
        } else {
            recommendations.push(TrainingRecommendationResponse {
                key: TrainingRecommendationKey::MaintainDhMomentum,
                priority: TrainingRecommendationPriority::Low,
                title: "Maintain downhill momentum".to_string(),
                detail: "Repeatability, recent top-3 pace, and lap count are all in a healthy place, so keep feeding the DH history with more marked sessions.".to_string(),
            });
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
            }),
            None,
            now,
        );

        let event_goal = response.event_goal.expect("event goal present");
        assert_eq!(event_goal.start_date, "2026-04-01");
        assert_eq!(event_goal.target_date, "2026-09-20");
        assert_eq!(event_goal.days_remaining, 122);
        assert_eq!(event_goal.training_window_days, 173);
        assert_eq!(event_goal.counted_ride_count, 2);
        assert_eq!(event_goal.counted_distance_meters, 152_000.0);
        assert_eq!(event_goal.counted_distance_progress_percent, 94.4);
        assert_eq!(event_goal.counted_elevation_gain_meters, 4_600.0);
        assert_eq!(event_goal.counted_elevation_gain_progress_percent, 100.0);
        assert_eq!(event_goal.best_distance_meters, Some(120_000.0));
        assert_eq!(event_goal.best_distance_progress_percent, Some(74.6));
        assert_eq!(event_goal.best_elevation_gain_meters, Some(2_700.0));
        assert_eq!(event_goal.best_elevation_gain_progress_percent, Some(68.1));
        assert_eq!(
            event_goal
                .best_distance_activity
                .as_ref()
                .map(|activity| activity.activity_id),
            Some(2)
        );
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
