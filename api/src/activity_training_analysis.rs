use crate::activity_details::{
    deserialize_derived_activity_data, ActivityChartPoint, ActivityRoutePoint,
};
use crate::app_error::AppError;
use crate::entities::{activities, activity_training_analyses, segment_efforts, segments};
use crate::training_profile::deserialize_activity_heart_rate_zones;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    Set,
};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use utoipa::ToSchema;

const CLIMB_MIN_GRADE_PERCENT: f64 = 3.0;
const CLIMB_MIN_GAIN_METERS: f64 = 20.0;
const CLIMB_MIN_DURATION_SECONDS: i32 = 60;
const CLIMB_MIN_DISTANCE_METERS: f64 = 250.0;
const DH_SESSION_MIN_SEGMENT_EFFORT_COUNT: i32 = 2;
const DH_SESSION_MIN_SEGMENT_TIME_SECONDS: i32 = 360;
const XC_ENDURANCE_MIN_TOTAL_TIME_SECONDS: i32 = 5_400;
const XC_ENDURANCE_MIN_Z2_TIME_SECONDS: i32 = 2_700;
const XC_ENDURANCE_MIN_DISTANCE_METERS: f64 = 25_000.0;
const XC_ENDURANCE_MIN_CLIMBING_GAIN_METERS: f64 = 400.0;
const XC_ENDURANCE_MIN_CLIMBING_TIME_SECONDS: i32 = 1_200;
const MIXED_XC_MIN_Z2_TIME_SECONDS: i32 = 1_200;
const MIXED_XC_MIN_DISTANCE_METERS: f64 = 15_000.0;
const MIXED_XC_MIN_CLIMBING_GAIN_METERS: f64 = 250.0;
const MIXED_XC_MIN_CLIMBING_TIME_SECONDS: i32 = 600;
const DISTANCE_COMPARISON_BUCKET_METERS: f64 = 5_000.0;
const ELEVATION_COMPARISON_BUCKET_METERS: f64 = 100.0;
const DECOUPLING_MIN_QUALIFYING_TIME_SECONDS: f64 = 1_800.0;
const DECOUPLING_MIN_PLAUSIBLE_PERCENT: f64 = -50.0;
const DECOUPLING_MAX_PLAUSIBLE_PERCENT: f64 = 50.0;
const ROUTE_FAMILY_MAX_TOKENS: usize = 4;
const TRAINING_ANALYSIS_BACKFILL_BATCH_SIZE: usize = 128;
const ROUTE_FAMILY_STOP_WORDS: &[&str] = &[
    "ride",
    "rides",
    "morning",
    "afternoon",
    "evening",
    "lunch",
    "night",
    "noon",
    "commute",
    "workout",
    "session",
    "test",
    "trainer",
    "indoor",
    "indoors",
    "outdoor",
    "outside",
    "recovery",
    "bike",
    "biking",
    "mtb",
    "xc",
    "dh",
    "lap",
    "laps",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityRideFocus {
    XcEndurance,
    MixedXc,
    DhSession,
    Other,
}

impl ActivityRideFocus {
    fn as_str(self) -> &'static str {
        match self {
            Self::XcEndurance => "xc_endurance",
            Self::MixedXc => "mixed_xc",
            Self::DhSession => "dh_session",
            Self::Other => "other",
        }
    }

    pub(crate) fn from_stored(value: &str) -> Self {
        match value {
            "xc_endurance" => Self::XcEndurance,
            "mixed_xc" => Self::MixedXc,
            "dh_session" => Self::DhSession,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct ActivityTrainingAnalysisResponse {
    pub ride_focus: ActivityRideFocus,
    pub route_family_key: Option<String>,
    pub comparable_distance_bucket_meters: Option<i32>,
    pub comparable_elevation_gain_bucket_meters: Option<i32>,
    pub aerobic_decoupling_percent: Option<f64>,
    pub z2_time_seconds: i32,
    pub z2_distance_meters: Option<f64>,
    pub z2_average_speed_mps: Option<f64>,
    pub climbing_time_seconds: i32,
    pub climbing_elevation_gain_meters: Option<f64>,
    pub sustained_climb_count: i32,
}

#[derive(Debug, Default, Clone, Copy)]
struct ActivityTrainingAnalysisContext {
    dh_segment_effort_count: i32,
    dh_segment_time_seconds: i32,
}

#[derive(Debug, Default, Clone, Copy)]
struct WeightedMetricHalf {
    duration_seconds: f64,
    weighted_speed_seconds: f64,
    weighted_heart_rate_seconds: f64,
}

#[derive(Debug, Clone, Copy)]
struct AerobicInterval {
    duration_seconds: f64,
    speed_mps: f64,
    heart_rate_bpm: f64,
}

#[derive(Debug, Clone, Copy)]
struct AnalysisSample {
    elapsed_seconds: i32,
    distance_meters: Option<f64>,
    elevation_meters: Option<f64>,
    heart_rate_bpm: Option<i32>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct ClimbAccumulator {
    duration_seconds: i32,
    elevation_gain_meters: f64,
    distance_meters: f64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct ClimbMetrics {
    climbing_time_seconds: i32,
    climbing_elevation_gain_meters: Option<f64>,
    sustained_climb_count: i32,
}

pub fn build_activity_training_analysis(
    activity: &activities::Model,
) -> ActivityTrainingAnalysisResponse {
    build_activity_training_analysis_with_context(
        activity,
        ActivityTrainingAnalysisContext::default(),
    )
}

fn build_activity_training_analysis_with_context(
    activity: &activities::Model,
    context: ActivityTrainingAnalysisContext,
) -> ActivityTrainingAnalysisResponse {
    let derived_data = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let samples =
        preferred_analysis_samples(&derived_data.route_points, &derived_data.chart_points);
    let z2_bounds = z2_bounds(activity);
    let z2_time_seconds = z2_time_seconds(activity);
    let z2_distance_meters =
        z2_bounds.and_then(|(min_bpm, max_bpm)| z2_distance_meters(&samples, min_bpm, max_bpm));
    let z2_average_speed_mps = z2_distance_meters.and_then(|distance_meters| {
        if z2_time_seconds > 0 {
            Some(distance_meters / f64::from(z2_time_seconds))
        } else {
            None
        }
    });
    let climb_metrics = climb_metrics(&samples);
    let ride_focus = classify_ride_focus(activity, z2_time_seconds, climb_metrics, context);
    let route_family_key = route_family_key(&activity.title);
    let comparable_distance_bucket_meters =
        distance_bucket_for_comparison(activity.distance_meters);
    let comparable_elevation_gain_bucket_meters = elevation_gain_bucket_for_comparison(
        activity
            .elevation_gain_meters
            .or(climb_metrics.climbing_elevation_gain_meters),
    );
    let is_comparable_endurance_ride = ride_focus == ActivityRideFocus::XcEndurance
        && route_family_key.is_some()
        && comparable_distance_bucket_meters.is_some()
        && comparable_elevation_gain_bucket_meters.is_some();
    let aerobic_decoupling_percent = z2_bounds.and_then(|(min_bpm, max_bpm)| {
        aerobic_decoupling_percent(&samples, min_bpm, max_bpm, is_comparable_endurance_ride)
    });

    ActivityTrainingAnalysisResponse {
        ride_focus,
        route_family_key,
        comparable_distance_bucket_meters,
        comparable_elevation_gain_bucket_meters,
        aerobic_decoupling_percent,
        z2_time_seconds,
        z2_distance_meters,
        z2_average_speed_mps,
        climbing_time_seconds: climb_metrics.climbing_time_seconds,
        climbing_elevation_gain_meters: climb_metrics.climbing_elevation_gain_meters,
        sustained_climb_count: climb_metrics.sustained_climb_count,
    }
}

pub async fn rebuild_activity_training_analysis_cache<C>(
    db: &C,
    activity_ids: &[i32],
) -> Result<(), sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let mut activity_ids = activity_ids
        .iter()
        .copied()
        .filter(|activity_id| *activity_id > 0)
        .collect::<Vec<_>>();
    activity_ids.sort_unstable();
    activity_ids.dedup();

    if activity_ids.is_empty() {
        return Ok(());
    }

    let contexts_by_activity_id =
        load_activity_training_analysis_contexts(db, &activity_ids).await?;

    let activity_models = activities::Entity::find()
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .all(db)
        .await?;
    let existing_models_by_activity_id = activity_training_analyses::Entity::find()
        .filter(activity_training_analyses::Column::ActivityId.is_in(activity_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|model| (model.activity_id, model))
        .collect::<HashMap<_, _>>();

    for activity in activity_models {
        let analysis = build_activity_training_analysis_with_context(
            &activity,
            contexts_by_activity_id
                .get(&activity.id)
                .copied()
                .unwrap_or_default(),
        );

        if let Some(existing_model) = existing_models_by_activity_id.get(&activity.id).cloned() {
            let mut active_model: activity_training_analyses::ActiveModel = existing_model.into();
            active_model.user_id = Set(activity.user_id);
            active_model.ride_focus = Set(analysis.ride_focus.as_str().to_string());
            active_model.route_family_key = Set(analysis.route_family_key.clone());
            active_model.comparable_distance_bucket_meters =
                Set(analysis.comparable_distance_bucket_meters);
            active_model.comparable_elevation_gain_bucket_meters =
                Set(analysis.comparable_elevation_gain_bucket_meters);
            active_model.aerobic_decoupling_percent = Set(analysis.aerobic_decoupling_percent);
            active_model.z2_time_seconds = Set(analysis.z2_time_seconds);
            active_model.z2_distance_meters = Set(analysis.z2_distance_meters);
            active_model.z2_average_speed_mps = Set(analysis.z2_average_speed_mps);
            active_model.climbing_time_seconds = Set(analysis.climbing_time_seconds);
            active_model.climbing_elevation_gain_meters =
                Set(analysis.climbing_elevation_gain_meters);
            active_model.sustained_climb_count = Set(analysis.sustained_climb_count);
            active_model.update(db).await?;
        } else {
            activity_training_analyses::ActiveModel {
                activity_id: Set(activity.id),
                user_id: Set(activity.user_id),
                ride_focus: Set(analysis.ride_focus.as_str().to_string()),
                route_family_key: Set(analysis.route_family_key.clone()),
                comparable_distance_bucket_meters: Set(analysis.comparable_distance_bucket_meters),
                comparable_elevation_gain_bucket_meters: Set(
                    analysis.comparable_elevation_gain_bucket_meters
                ),
                aerobic_decoupling_percent: Set(analysis.aerobic_decoupling_percent),
                z2_time_seconds: Set(analysis.z2_time_seconds),
                z2_distance_meters: Set(analysis.z2_distance_meters),
                z2_average_speed_mps: Set(analysis.z2_average_speed_mps),
                climbing_time_seconds: Set(analysis.climbing_time_seconds),
                climbing_elevation_gain_meters: Set(analysis.climbing_elevation_gain_meters),
                sustained_climb_count: Set(analysis.sustained_climb_count),
                ..Default::default()
            }
            .insert(db)
            .await?;
        }
    }

    Ok(())
}

pub async fn backfill_user_activity_training_analysis_cache(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<usize, AppError> {
    let activity_ids = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .all(db)
        .await?
        .into_iter()
        .map(|activity| activity.id)
        .collect::<Vec<_>>();

    for activity_id_batch in activity_ids.chunks(TRAINING_ANALYSIS_BACKFILL_BATCH_SIZE) {
        rebuild_activity_training_analysis_cache(db, activity_id_batch).await?;
    }

    Ok(activity_ids.len())
}

pub async fn load_activity_training_analysis_by_activity_id<C>(
    db: &C,
    activity_id: i32,
) -> Result<Option<ActivityTrainingAnalysisResponse>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    Ok(activity_training_analyses::Entity::find_by_id(activity_id)
        .one(db)
        .await?
        .map(response_from_model))
}

fn response_from_model(
    model: activity_training_analyses::Model,
) -> ActivityTrainingAnalysisResponse {
    ActivityTrainingAnalysisResponse {
        ride_focus: ActivityRideFocus::from_stored(&model.ride_focus),
        route_family_key: model.route_family_key,
        comparable_distance_bucket_meters: model.comparable_distance_bucket_meters,
        comparable_elevation_gain_bucket_meters: model.comparable_elevation_gain_bucket_meters,
        aerobic_decoupling_percent: model.aerobic_decoupling_percent,
        z2_time_seconds: model.z2_time_seconds,
        z2_distance_meters: model.z2_distance_meters,
        z2_average_speed_mps: model.z2_average_speed_mps,
        climbing_time_seconds: model.climbing_time_seconds,
        climbing_elevation_gain_meters: model.climbing_elevation_gain_meters,
        sustained_climb_count: model.sustained_climb_count,
    }
}

async fn load_activity_training_analysis_contexts<C>(
    db: &C,
    activity_ids: &[i32],
) -> Result<HashMap<i32, ActivityTrainingAnalysisContext>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    let effort_models = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::ActivityId.is_in(activity_ids.iter().copied()))
        .all(db)
        .await?;
    if effort_models.is_empty() {
        return Ok(HashMap::new());
    }

    let mut segment_ids = effort_models
        .iter()
        .map(|effort| effort.segment_id)
        .collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();

    let dh_segment_ids = segments::Entity::find()
        .filter(segments::Column::Id.is_in(segment_ids.iter().copied()))
        .filter(segments::Column::Mode.eq("dh"))
        .all(db)
        .await?
        .into_iter()
        .map(|segment| segment.id)
        .collect::<HashSet<_>>();
    if dh_segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut contexts_by_activity_id = HashMap::<i32, ActivityTrainingAnalysisContext>::new();

    for effort in effort_models {
        if !dh_segment_ids.contains(&effort.segment_id) {
            continue;
        }

        let context = contexts_by_activity_id
            .entry(effort.activity_id)
            .or_default();
        context.dh_segment_effort_count += 1;
        context.dh_segment_time_seconds += effort.duration_seconds.max(0);
    }

    Ok(contexts_by_activity_id)
}

fn preferred_analysis_samples(
    route_points: &[ActivityRoutePoint],
    chart_points: &[ActivityChartPoint],
) -> Vec<AnalysisSample> {
    if route_points.len() >= 2 {
        return route_points
            .iter()
            .map(|point| AnalysisSample {
                elapsed_seconds: point.elapsed_seconds,
                distance_meters: point.distance_meters,
                elevation_meters: point.elevation_meters,
                heart_rate_bpm: point.heart_rate_bpm,
            })
            .collect();
    }

    chart_points
        .iter()
        .map(|point| AnalysisSample {
            elapsed_seconds: point.elapsed_seconds,
            distance_meters: point.distance_meters,
            elevation_meters: point.elevation_meters,
            heart_rate_bpm: point.heart_rate_bpm,
        })
        .collect()
}

fn z2_bounds(activity: &activities::Model) -> Option<(i32, Option<i32>)> {
    deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref())
        .into_iter()
        .find(|zone| zone.zone == 2)
        .and_then(|zone| Some((zone.min_bpm?, zone.max_bpm)))
}

fn z2_time_seconds(activity: &activities::Model) -> i32 {
    deserialize_activity_heart_rate_zones(activity.heart_rate_zones_json.as_ref())
        .into_iter()
        .find(|zone| zone.zone == 2)
        .map(|zone| zone.duration_seconds)
        .unwrap_or_default()
}

fn z2_distance_meters(
    samples: &[AnalysisSample],
    min_bpm: i32,
    max_bpm: Option<i32>,
) -> Option<f64> {
    let mut total_distance_meters = 0.0;
    let mut found = false;

    for window in samples.windows(2) {
        let previous = window[0];
        let current = window[1];
        let (Some(previous_distance), Some(current_distance)) =
            (previous.distance_meters, current.distance_meters)
        else {
            continue;
        };

        if current_distance < previous_distance {
            continue;
        }

        let heart_rate_bpm = averaged_heart_rate(previous.heart_rate_bpm, current.heart_rate_bpm);

        if !heart_rate_in_zone(heart_rate_bpm, min_bpm, max_bpm) {
            continue;
        }

        total_distance_meters += current_distance - previous_distance;
        found = true;
    }

    found.then_some(total_distance_meters)
}

pub fn plausible_aerobic_decoupling_percent(value: f64) -> Option<f64> {
    (value.is_finite()
        && (DECOUPLING_MIN_PLAUSIBLE_PERCENT..=DECOUPLING_MAX_PLAUSIBLE_PERCENT).contains(&value))
    .then_some(value)
}

fn averaged_heart_rate(previous: Option<i32>, current: Option<i32>) -> Option<i32> {
    match (previous, current) {
        (Some(left), Some(right)) => Some((left + right) / 2),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

fn heart_rate_in_zone(value: Option<i32>, min_bpm: i32, max_bpm: Option<i32>) -> bool {
    let Some(value) = value else {
        return false;
    };

    if value < min_bpm {
        return false;
    }

    max_bpm.is_none_or(|ceiling| value <= ceiling)
}

fn classify_ride_focus(
    activity: &activities::Model,
    z2_time_seconds: i32,
    climb_metrics: ClimbMetrics,
    context: ActivityTrainingAnalysisContext,
) -> ActivityRideFocus {
    if context.dh_segment_effort_count >= DH_SESSION_MIN_SEGMENT_EFFORT_COUNT
        && context.dh_segment_time_seconds >= DH_SESSION_MIN_SEGMENT_TIME_SECONDS
    {
        return ActivityRideFocus::DhSession;
    }

    let total_time_seconds = activity
        .moving_time_seconds
        .or(activity.total_time_seconds)
        .unwrap_or_default();
    let distance_meters = activity.distance_meters.unwrap_or_default();
    let climbing_gain_meters = activity
        .elevation_gain_meters
        .or(climb_metrics.climbing_elevation_gain_meters)
        .unwrap_or_default();
    let qualifies_xc_endurance = total_time_seconds >= XC_ENDURANCE_MIN_TOTAL_TIME_SECONDS
        && (z2_time_seconds >= XC_ENDURANCE_MIN_Z2_TIME_SECONDS
            || (distance_meters >= XC_ENDURANCE_MIN_DISTANCE_METERS
                && climbing_gain_meters >= XC_ENDURANCE_MIN_CLIMBING_GAIN_METERS)
            || climb_metrics.climbing_time_seconds >= XC_ENDURANCE_MIN_CLIMBING_TIME_SECONDS);

    if qualifies_xc_endurance {
        return ActivityRideFocus::XcEndurance;
    }

    let qualifies_mixed_xc = z2_time_seconds >= MIXED_XC_MIN_Z2_TIME_SECONDS
        || (distance_meters >= MIXED_XC_MIN_DISTANCE_METERS
            && climbing_gain_meters >= MIXED_XC_MIN_CLIMBING_GAIN_METERS)
        || climb_metrics.climbing_time_seconds >= MIXED_XC_MIN_CLIMBING_TIME_SECONDS
        || climb_metrics.sustained_climb_count >= 2;

    if qualifies_mixed_xc {
        return ActivityRideFocus::MixedXc;
    }

    ActivityRideFocus::Other
}

fn route_family_key(title: &str) -> Option<String> {
    let tokens = title
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter_map(normalize_route_family_token)
        .take(ROUTE_FAMILY_MAX_TOKENS)
        .collect::<Vec<_>>();

    if tokens.is_empty() {
        return None;
    }

    Some(tokens.join("-"))
}

fn normalize_route_family_token(raw_token: &str) -> Option<String> {
    if raw_token.is_empty() {
        return None;
    }

    let token = raw_token.to_ascii_lowercase();

    if token.len() < 2
        || token.chars().all(|character| character.is_ascii_digit())
        || token.chars().any(|character| character.is_ascii_digit())
        || ROUTE_FAMILY_STOP_WORDS.contains(&token.as_str())
    {
        return None;
    }

    Some(token)
}

fn distance_bucket_for_comparison(distance_meters: Option<f64>) -> Option<i32> {
    comparable_bucket(distance_meters, DISTANCE_COMPARISON_BUCKET_METERS)
}

fn elevation_gain_bucket_for_comparison(elevation_gain_meters: Option<f64>) -> Option<i32> {
    comparable_bucket(elevation_gain_meters, ELEVATION_COMPARISON_BUCKET_METERS)
}

fn comparable_bucket(value: Option<f64>, bucket_size: f64) -> Option<i32> {
    let value = value?;
    if value <= 0.0 {
        return None;
    }

    let rounded_bucket = (value / bucket_size).round().max(1.0) * bucket_size;

    Some(rounded_bucket as i32)
}

impl WeightedMetricHalf {
    fn add(&mut self, duration_seconds: f64, speed_mps: f64, heart_rate_bpm: f64) {
        if duration_seconds <= 0.0 {
            return;
        }

        self.duration_seconds += duration_seconds;
        self.weighted_speed_seconds += speed_mps * duration_seconds;
        self.weighted_heart_rate_seconds += heart_rate_bpm * duration_seconds;
    }

    fn efficiency_factor(self) -> Option<f64> {
        if self.duration_seconds <= 0.0
            || self.weighted_speed_seconds <= 0.0
            || self.weighted_heart_rate_seconds <= 0.0
        {
            return None;
        }

        let average_speed_mps = self.weighted_speed_seconds / self.duration_seconds;
        let average_heart_rate_bpm = self.weighted_heart_rate_seconds / self.duration_seconds;

        (average_heart_rate_bpm > 0.0).then_some(average_speed_mps / average_heart_rate_bpm)
    }
}

fn aerobic_decoupling_percent(
    samples: &[AnalysisSample],
    min_bpm: i32,
    max_bpm: Option<i32>,
    is_comparable_endurance_ride: bool,
) -> Option<f64> {
    if !is_comparable_endurance_ride {
        return None;
    }

    let intervals = aerobic_intervals(samples, min_bpm, max_bpm);
    if intervals.is_empty() {
        return None;
    }

    let total_duration_seconds = intervals
        .iter()
        .map(|interval| interval.duration_seconds)
        .sum::<f64>();
    if total_duration_seconds < DECOUPLING_MIN_QUALIFYING_TIME_SECONDS {
        return None;
    }

    let midpoint_seconds = total_duration_seconds / 2.0;
    let mut elapsed_seconds = 0.0;
    let mut first_half = WeightedMetricHalf::default();
    let mut second_half = WeightedMetricHalf::default();

    for interval in intervals {
        let interval_end_seconds = elapsed_seconds + interval.duration_seconds;
        let first_half_duration_seconds =
            (midpoint_seconds.min(interval_end_seconds) - elapsed_seconds).max(0.0);
        let second_half_duration_seconds =
            (interval.duration_seconds - first_half_duration_seconds).max(0.0);

        first_half.add(
            first_half_duration_seconds,
            interval.speed_mps,
            interval.heart_rate_bpm,
        );
        second_half.add(
            second_half_duration_seconds,
            interval.speed_mps,
            interval.heart_rate_bpm,
        );
        elapsed_seconds = interval_end_seconds;
    }

    let first_efficiency = first_half.efficiency_factor()?;
    let second_efficiency = second_half.efficiency_factor()?;

    plausible_aerobic_decoupling_percent(
        ((first_efficiency - second_efficiency) / first_efficiency) * 100.0,
    )
}

fn aerobic_intervals(
    samples: &[AnalysisSample],
    min_bpm: i32,
    max_bpm: Option<i32>,
) -> Vec<AerobicInterval> {
    samples
        .windows(2)
        .filter_map(|window| {
            let previous = window[0];
            let current = window[1];
            let delta_time_seconds = current.elapsed_seconds - previous.elapsed_seconds;
            let delta_distance_meters = current.distance_meters? - previous.distance_meters?;
            let heart_rate_bpm =
                averaged_heart_rate(previous.heart_rate_bpm, current.heart_rate_bpm)?;

            if delta_time_seconds <= 0
                || delta_distance_meters <= 0.0
                || !heart_rate_in_zone(Some(heart_rate_bpm), min_bpm, max_bpm)
            {
                return None;
            }

            let duration_seconds = f64::from(delta_time_seconds);
            let speed_mps = delta_distance_meters / duration_seconds;

            if !speed_mps.is_finite() || speed_mps <= 0.0 {
                return None;
            }

            Some(AerobicInterval {
                duration_seconds,
                speed_mps,
                heart_rate_bpm: f64::from(heart_rate_bpm),
            })
        })
        .collect()
}

fn climb_metrics(samples: &[AnalysisSample]) -> ClimbMetrics {
    let mut totals = ClimbMetrics::default();
    let mut block = ClimbAccumulator::default();

    for window in samples.windows(2) {
        let previous = window[0];
        let current = window[1];

        if let Some((delta_time_seconds, delta_distance_meters, delta_gain_meters)) =
            qualifying_climb_delta(previous, current)
        {
            block.duration_seconds += delta_time_seconds;
            block.distance_meters += delta_distance_meters;
            block.elevation_gain_meters += delta_gain_meters;
            continue;
        }

        finalize_climb_block(&mut totals, &mut block);
    }

    finalize_climb_block(&mut totals, &mut block);

    totals
}

fn qualifying_climb_delta(
    previous: AnalysisSample,
    current: AnalysisSample,
) -> Option<(i32, f64, f64)> {
    let delta_time_seconds = current.elapsed_seconds - previous.elapsed_seconds;
    let delta_distance_meters = current.distance_meters? - previous.distance_meters?;
    let delta_elevation_meters = current.elevation_meters? - previous.elevation_meters?;

    if delta_time_seconds <= 0 || delta_distance_meters <= 0.0 || delta_elevation_meters <= 0.0 {
        return None;
    }

    let grade_percent = (delta_elevation_meters / delta_distance_meters) * 100.0;

    if grade_percent < CLIMB_MIN_GRADE_PERCENT {
        return None;
    }

    Some((
        delta_time_seconds,
        delta_distance_meters,
        delta_elevation_meters,
    ))
}

fn finalize_climb_block(totals: &mut ClimbMetrics, block: &mut ClimbAccumulator) {
    let qualifies = block.duration_seconds >= CLIMB_MIN_DURATION_SECONDS
        && block.distance_meters >= CLIMB_MIN_DISTANCE_METERS
        && block.elevation_gain_meters >= CLIMB_MIN_GAIN_METERS;

    if qualifies {
        totals.climbing_time_seconds += block.duration_seconds;
        totals.climbing_elevation_gain_meters = Some(
            totals.climbing_elevation_gain_meters.unwrap_or(0.0) + block.elevation_gain_meters,
        );
        totals.sustained_climb_count += 1;
    }

    *block = ClimbAccumulator::default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_details::{serialize_derived_activity_data, ActivityDerivedData};
    use crate::training_profile::{
        serialize_activity_heart_rate_zones, ActivityHeartRateZoneSummary,
    };
    use chrono::Utc;

    fn build_route_point(
        elapsed_seconds: i32,
        distance_meters: f64,
        elevation_meters: f64,
        heart_rate_bpm: i32,
    ) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds,
            latitude: 45.0 + f64::from(elapsed_seconds) * 0.0001,
            longitude: -122.0,
            distance_meters: Some(distance_meters),
            elevation_meters: Some(elevation_meters),
            speed_mps: None,
            heart_rate_bpm: Some(heart_rate_bpm),
            cadence_rpm: None,
            power_watts: None,
        }
    }

    fn total_elevation_gain_meters(route_points: &[ActivityRoutePoint]) -> f64 {
        route_points
            .windows(2)
            .map(|window| {
                let previous = window[0].elevation_meters.unwrap_or_default();
                let current = window[1].elevation_meters.unwrap_or_default();
                (current - previous).max(0.0)
            })
            .sum()
    }

    fn build_activity(title: &str, route_points: Vec<ActivityRoutePoint>) -> activities::Model {
        build_activity_with_z2_duration(title, route_points, 120)
    }

    fn build_activity_with_z2_duration(
        title: &str,
        route_points: Vec<ActivityRoutePoint>,
        z2_duration_seconds: i32,
    ) -> activities::Model {
        let now = Utc::now();
        let total_elevation_gain_meters = total_elevation_gain_meters(&route_points);

        activities::Model {
            id: 7,
            user_id: 1,
            activity_import_id: None,
            title: title.to_string(),
            sport: "ride".to_string(),
            source: "manual_upload".to_string(),
            source_correlation_id: None,
            original_filename: Some("ride.fit".to_string()),
            format: Some("fit".to_string()),
            activity_type: crate::activity_type::ActivityType::Training
                .as_str()
                .to_string(),
            started_at: now,
            ended_at: Some(now),
            distance_meters: route_points.last().and_then(|point| point.distance_meters),
            moving_time_seconds: Some(
                route_points
                    .last()
                    .map(|point| point.elapsed_seconds)
                    .unwrap_or_default(),
            ),
            total_time_seconds: Some(
                route_points
                    .last()
                    .map(|point| point.elapsed_seconds)
                    .unwrap_or_default(),
            ),
            elevation_gain_meters: Some(total_elevation_gain_meters),
            elevation_loss_meters: Some(total_elevation_gain_meters),
            average_speed_mps: Some(6.0),
            max_speed_mps: Some(10.0),
            average_heart_rate_bpm: Some(136),
            max_heart_rate_bpm: Some(156),
            average_cadence_rpm: None,
            max_cadence_rpm: None,
            calories: None,
            estimated_ftp_watts: None,
            heart_rate_zones_json: serialize_activity_heart_rate_zones(&[
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
                    duration_seconds: z2_duration_seconds,
                    share_percent: 50.0,
                },
            ])
            .expect("serialize heart rate zones"),
            derived_data_json: Some(
                serialize_derived_activity_data(&ActivityDerivedData {
                    laps: Vec::new(),
                    chart_points: Vec::new(),
                    route_points,
                })
                .expect("serialize derived data"),
            ),
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn builds_z2_metrics_from_activity_samples() {
        let activity = build_activity(
            "Morning Ride",
            vec![
                build_route_point(0, 0.0, 100.0, 118),
                build_route_point(60, 500.0, 110.0, 130),
                build_route_point(120, 1200.0, 140.0, 136),
                build_route_point(180, 1800.0, 145.0, 150),
            ],
        );

        let analysis = build_activity_training_analysis(&activity);

        assert_eq!(analysis.ride_focus, ActivityRideFocus::Other);
        assert_eq!(analysis.route_family_key, None);
        assert_eq!(analysis.comparable_distance_bucket_meters, Some(5_000));
        assert_eq!(analysis.comparable_elevation_gain_bucket_meters, Some(100));
        assert_eq!(analysis.aerobic_decoupling_percent, None);
        assert_eq!(analysis.z2_time_seconds, 120);
        assert_eq!(analysis.z2_distance_meters, Some(1200.0));
        assert_eq!(analysis.z2_average_speed_mps, Some(10.0));
    }

    #[test]
    fn counts_sustained_climbs_from_route_points() {
        let activity = build_activity(
            "Post Canyon Endurance Lap 3",
            vec![
                build_route_point(0, 0.0, 100.0, 130),
                build_route_point(900, 3_000.0, 300.0, 132),
                build_route_point(1_800, 6_000.0, 520.0, 134),
                build_route_point(2_700, 9_000.0, 760.0, 136),
                build_route_point(3_600, 12_000.0, 760.0, 138),
                build_route_point(4_500, 15_000.0, 960.0, 139),
                build_route_point(5_400, 18_000.0, 1_180.0, 140),
                build_route_point(6_300, 21_000.0, 1_400.0, 141),
            ],
        );

        let analysis = build_activity_training_analysis(&activity);

        assert_eq!(analysis.ride_focus, ActivityRideFocus::XcEndurance);
        assert_eq!(
            analysis.route_family_key.as_deref(),
            Some("post-canyon-endurance")
        );
        assert_eq!(analysis.comparable_distance_bucket_meters, Some(20_000));
        assert_eq!(
            analysis.comparable_elevation_gain_bucket_meters,
            Some(1_300)
        );
        assert!(analysis.aerobic_decoupling_percent.is_some());
        assert_eq!(analysis.sustained_climb_count, 2);
        assert_eq!(analysis.climbing_time_seconds, 5_400);
        assert_eq!(analysis.climbing_elevation_gain_meters, Some(1_300.0));
    }

    #[test]
    fn classifies_dh_sessions_from_dh_segment_context() {
        let activity = build_activity(
            "FMR laps",
            vec![
                build_route_point(0, 0.0, 1_000.0, 118),
                build_route_point(300, 2_500.0, 900.0, 130),
                build_route_point(600, 5_000.0, 800.0, 132),
            ],
        );

        let analysis = build_activity_training_analysis_with_context(
            &activity,
            ActivityTrainingAnalysisContext {
                dh_segment_effort_count: 3,
                dh_segment_time_seconds: 480,
            },
        );

        assert_eq!(analysis.ride_focus, ActivityRideFocus::DhSession);
        assert_eq!(analysis.route_family_key.as_deref(), Some("fmr"));
        assert_eq!(analysis.comparable_distance_bucket_meters, Some(5_000));
        assert_eq!(analysis.aerobic_decoupling_percent, None);
    }

    #[test]
    fn calculates_aerobic_decoupling_for_comparable_xc_endurance_rides() {
        let activity = build_activity_with_z2_duration(
            "Post Canyon Endurance",
            vec![
                build_route_point(0, 0.0, 100.0, 130),
                build_route_point(900, 9_000.0, 250.0, 130),
                build_route_point(1_800, 18_000.0, 400.0, 130),
                build_route_point(2_700, 27_000.0, 550.0, 130),
                build_route_point(3_600, 36_000.0, 700.0, 130),
                build_route_point(4_500, 44_100.0, 850.0, 140),
                build_route_point(5_400, 52_200.0, 1_000.0, 140),
                build_route_point(6_300, 60_300.0, 1_150.0, 140),
                build_route_point(7_200, 68_400.0, 1_300.0, 140),
            ],
            5_400,
        );

        let analysis = build_activity_training_analysis(&activity);

        assert_eq!(analysis.ride_focus, ActivityRideFocus::XcEndurance);
        assert_eq!(
            analysis.route_family_key.as_deref(),
            Some("post-canyon-endurance")
        );
        assert_eq!(analysis.comparable_distance_bucket_meters, Some(70_000));
        assert_eq!(
            analysis.comparable_elevation_gain_bucket_meters,
            Some(1_200)
        );
        assert!(analysis.aerobic_decoupling_percent.is_some());
        assert!((analysis.aerobic_decoupling_percent.unwrap_or_default() - 15.68).abs() < 0.2);
    }

    #[test]
    fn rejects_implausible_aerobic_decoupling_values() {
        assert_eq!(plausible_aerobic_decoupling_percent(-189.3), None);
        assert_eq!(plausible_aerobic_decoupling_percent(88.0), None);
        assert_eq!(plausible_aerobic_decoupling_percent(f64::NAN), None);
        assert_eq!(plausible_aerobic_decoupling_percent(-21.5), Some(-21.5));
        assert_eq!(plausible_aerobic_decoupling_percent(28.9), Some(28.9));
    }
}
