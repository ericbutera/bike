use crate::activity_location::location_from_derived_json;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bike_core::activity_achievements::ActivityAchievementHighlight;
use bike_core::activity_details::{
    deserialize_derived_activity_data, ActivityChartPoint, ActivityDerivedData, ActivityLap, ActivityRoutePoint,
};
use bike_core::activity_lifecycle::delete_activity_with_derived_state;
use bike_core::activity_sport::ActivitySport;
use bike_core::activity_training_analysis::{
    load_activity_training_analysis_by_activity_id, ActivityTrainingAnalysisResponse,
};
use bike_core::activity_type::ActivityType;
use bike_core::analytics::{estimated_training_load, mark_segment_activity_changes, mark_user_fitness_dirty};
use bike_core::auth::UserContext;
use bike_core::entities::{
    activities, activity_analytics, activity_import_artifacts, segment_efforts, segment_user_summaries, segments,
};
use bike_core::platform::data::pagination::{PaginatedResponse, PaginationParams};
use bike_core::training_profile::{deserialize_activity_heart_rate_zones, ActivityHeartRateZoneSummary};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

const MAX_ACTIVITY_STREAM_ROUTE_POINTS: usize = 24;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ActivityListParams {
    /// Page number (1-based)
    #[param(example = 1)]
    #[serde(
        default = "default_activity_page",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub page: i64,
    /// Number of items per page
    #[param(example = 20)]
    #[serde(
        default = "default_activity_per_page",
        deserialize_with = "deserialize_number_from_string"
    )]
    pub per_page: i64,
    /// Filter by normalized activity sport
    #[param(value_type = Option<ActivitySport>)]
    pub sport: Option<ActivitySport>,
}

impl ActivityListParams {
    fn pagination(&self) -> PaginationParams {
        PaginationParams {
            page: self.page,
            per_page: self.per_page,
        }
    }
}

fn default_activity_page() -> i64 {
    1
}

fn default_activity_per_page() -> i64 {
    20
}

fn deserialize_number_from_string<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        String(String),
        Int(i64),
    }

    match StringOrInt::deserialize(deserializer)? {
        StringOrInt::String(value) => value.parse().map_err(serde::de::Error::custom),
        StringOrInt::Int(value) => Ok(value),
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 42,"title": "Saturday hill repeats","sport": "ride","source": "manual_upload","activity_type": "training","original_filename": "saturday-hills.fit","format": "fit","started_at": "2026-09-26T13:00:00Z","ended_at": "2026-09-26T14:12:00Z","location": "Detroit, Michigan","distance_meters": 25430.5,"moving_time_seconds": 4200,"total_time_seconds": 4320,"elevation_gain_meters": 312.4,"average_speed_mps": 6.05,"average_heart_rate_bpm": 148,"estimated_ftp_watts": 245,"route_points": [{"elapsed_seconds": 900,"latitude": 42.3314,"longitude": -83.0458,"distance_meters": 4200.5,"elevation_meters": 183.2}],"can_regenerate": true,"can_download_source_file": true}))]
pub struct ActivityResponse {
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private_notes: Option<String>,
    pub gear: Option<bike_core::activity_data::ActivityGear>,
    pub activity_import_id: Option<i32>,
    #[schema(example = 42)]
    pub id: i32,
    #[schema(example = "Saturday hill repeats")]
    pub title: String,
    #[schema(example = "ride")]
    pub sport: String,
    #[schema(example = "manual_upload")]
    pub source: String,
    pub activity_type: ActivityType,
    #[schema(example = "saturday-hills.fit")]
    pub original_filename: Option<String>,
    #[schema(example = "fit")]
    pub format: Option<String>,
    #[schema(example = "2026-09-26T13:00:00Z")]
    pub started_at: DateTime<Utc>,
    #[schema(example = "2026-09-26T14:12:00Z")]
    pub ended_at: Option<DateTime<Utc>>,
    #[schema(example = "Detroit, Michigan")]
    pub location: Option<String>,
    #[schema(example = 25430.5)]
    pub distance_meters: Option<f64>,
    #[schema(example = 4200)]
    pub moving_time_seconds: Option<i32>,
    #[schema(example = 4320)]
    pub total_time_seconds: Option<i32>,
    #[schema(example = 312.4)]
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    #[schema(example = 6.05)]
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    #[schema(example = 148)]
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
    pub relative_effort: Option<i32>,
    #[schema(example = 245)]
    pub estimated_ftp_watts: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heart_rate_zones: Vec<ActivityHeartRateZoneSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schema(example = json!([{"elapsed_seconds": 900,"latitude": 42.3314,"longitude": -83.0458,"distance_meters": 4200.5,"elevation_meters": 183.2}]))]
    pub route_points: Vec<ActivityRoutePoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub achievement_highlights: Vec<ActivityAchievementHighlight>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segment_efforts: Vec<ActivitySegmentEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_analysis: Option<ActivityTrainingAnalysisResponse>,
    #[serde(default, skip_serializing_if = "is_false")]
    #[schema(example = true)]
    pub can_regenerate: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    #[schema(example = true)]
    pub can_download_source_file: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"title": "Saturday hill repeats","activity_type": "training"}))]
pub struct UpdateActivityRequest {
    #[schema(example = "Saturday hill repeats")]
    pub title: Option<String>,
    #[schema(example = "training")]
    pub activity_type: Option<ActivityType>,
    pub sport: Option<ActivitySport>,
    /// Empty text clears the manual location and restores route-derived location.
    pub location: Option<String>,
    /// Empty text clears the description.
    pub description: Option<String>,
    /// Empty text clears private notes.
    pub private_notes: Option<String>,
    /// Changes the existing gear's display name while preserving its identifier.
    pub gear_name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"segment_id": 7,"segment_title": "Riverfront climb","effort_index": 1,"duration_seconds": 355,"start_route_point_index": 120,"end_route_point_index": 280,"overall_rank": 3,"personal_rank": 1}))]
pub struct ActivitySegmentEffort {
    #[schema(example = 7)]
    pub segment_id: i32,
    #[schema(example = "Riverfront climb")]
    pub segment_title: String,
    #[schema(example = 1)]
    pub effort_index: i32,
    #[schema(example = 355)]
    pub duration_seconds: i32,
    #[schema(example = 120)]
    pub start_route_point_index: i32,
    #[schema(example = 280)]
    pub end_route_point_index: i32,
    #[schema(example = 3)]
    pub overall_rank: Option<i32>,
    #[schema(example = 1)]
    pub personal_rank: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl ActivityResponse {
    fn from_summary_with_achievement_highlights(
        model: activities::Model,
        achievement_highlights: Vec<ActivityAchievementHighlight>,
    ) -> Self {
        let derived_data = summary_derived_data(model.derived_data_json.as_ref());
        let location = location_from_derived_json(model.derived_data_json.as_ref());

        Self::from_model(model, derived_data, location, achievement_highlights, Vec::new(), false)
    }

    fn from_detail(model: activities::Model, segment_efforts: Vec<ActivitySegmentEffort>) -> Self {
        let can_regenerate = model.activity_import_id.is_some();
        let derived_data = deserialize_derived_activity_data(model.derived_data_json.as_ref());
        let location = location_from_derived_json(model.derived_data_json.as_ref());

        Self::from_model(
            model,
            derived_data,
            location,
            Vec::new(),
            segment_efforts,
            can_regenerate,
        )
    }

    fn from_model(
        model: activities::Model,
        derived_data: ActivityDerivedData,
        location: Option<String>,
        achievement_highlights: Vec<ActivityAchievementHighlight>,
        segment_efforts: Vec<ActivitySegmentEffort>,
        can_regenerate: bool,
    ) -> Self {
        let relative_effort = relative_effort(&model);
        let metadata = model
            .derived_data_json
            .as_ref()
            .and_then(|data| data.source_metadata())
            .cloned()
            .unwrap_or_default();

        Self {
            description: metadata.description,
            private_notes: metadata.private_notes,
            gear: metadata.gear,
            activity_import_id: model.activity_import_id,
            id: model.id,
            title: model.title,
            sport: model.sport,
            source: model.source,
            activity_type: ActivityType::from_stored(&model.activity_type),
            original_filename: model.original_filename,
            format: model.format,
            started_at: model.started_at,
            ended_at: model.ended_at,
            location,
            distance_meters: model.distance_meters,
            moving_time_seconds: model.moving_time_seconds,
            total_time_seconds: model.total_time_seconds,
            elevation_gain_meters: model.elevation_gain_meters,
            elevation_loss_meters: model.elevation_loss_meters,
            average_speed_mps: model.average_speed_mps,
            max_speed_mps: model.max_speed_mps,
            average_heart_rate_bpm: model.average_heart_rate_bpm,
            max_heart_rate_bpm: model.max_heart_rate_bpm,
            average_cadence_rpm: model.average_cadence_rpm,
            max_cadence_rpm: model.max_cadence_rpm,
            calories: model.calories,
            relative_effort,
            estimated_ftp_watts: model.estimated_ftp_watts,
            heart_rate_zones: deserialize_activity_heart_rate_zones(model.heart_rate_zones_json.as_ref()),
            laps: derived_data.laps,
            chart_points: derived_data.chart_points,
            route_points: derived_data.route_points,
            achievement_highlights,
            segment_efforts,
            training_analysis: None,
            can_regenerate,
            can_download_source_file: can_regenerate,
        }
    }
}

fn summary_derived_data(raw: Option<&bike_core::activity_details::StoredActivityDerivedData>) -> ActivityDerivedData {
    let derived_data = deserialize_derived_activity_data(raw);

    ActivityDerivedData {
        route_points: preview_route_points(&derived_data.route_points),
        ..Default::default()
    }
}

fn preview_route_points(route_points: &[ActivityRoutePoint]) -> Vec<ActivityRoutePoint> {
    if route_points.len() <= MAX_ACTIVITY_STREAM_ROUTE_POINTS {
        return route_points.to_vec();
    }

    let last_index = route_points.len().saturating_sub(1);
    let mut selected_indexes = BTreeSet::from([
        0,
        last_index,
        route_points
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.latitude.total_cmp(&right.latitude))
            .map(|(index, _)| index)
            .unwrap_or(0),
        route_points
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.latitude.total_cmp(&right.latitude))
            .map(|(index, _)| index)
            .unwrap_or(last_index),
        route_points
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.longitude.total_cmp(&right.longitude))
            .map(|(index, _)| index)
            .unwrap_or(0),
        route_points
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.longitude.total_cmp(&right.longitude))
            .map(|(index, _)| index)
            .unwrap_or(last_index),
    ]);

    for sample_index in 0..MAX_ACTIVITY_STREAM_ROUTE_POINTS {
        if selected_indexes.len() >= MAX_ACTIVITY_STREAM_ROUTE_POINTS {
            break;
        }

        let point_index = sample_index * last_index / (MAX_ACTIVITY_STREAM_ROUTE_POINTS.saturating_sub(1));
        selected_indexes.insert(point_index);
    }

    if selected_indexes.len() < MAX_ACTIVITY_STREAM_ROUTE_POINTS {
        for point_index in 0..route_points.len() {
            if selected_indexes.len() >= MAX_ACTIVITY_STREAM_ROUTE_POINTS {
                break;
            }

            selected_indexes.insert(point_index);
        }
    }

    selected_indexes
        .into_iter()
        .take(MAX_ACTIVITY_STREAM_ROUTE_POINTS)
        .map(|index| route_points[index].clone())
        .collect()
}

fn relative_effort(model: &activities::Model) -> Option<i32> {
    estimated_training_load(model).map(|value| value.round() as i32)
}

fn rank_efforts_by_segment(efforts: Vec<segment_efforts::Model>) -> HashMap<i32, i32> {
    let mut sorted_efforts = efforts;
    sorted_efforts.sort_by_key(|effort| (effort.segment_id, effort.duration_seconds, effort.id));

    let mut ranks = HashMap::<i32, i32>::new();
    let mut current_segment_id = None::<i32>;
    let mut current_rank = 0;

    for effort in sorted_efforts {
        let next_rank = if current_segment_id == Some(effort.segment_id) {
            current_rank + 1
        } else {
            1
        };

        ranks.insert(effort.id, next_rank);
        current_segment_id = Some(effort.segment_id);
        current_rank = next_rank;
    }

    ranks
}

fn overall_ranks_by_effort_id(efforts: &[segment_efforts::Model]) -> HashMap<i32, i32> {
    rank_efforts_by_segment(efforts.to_vec())
}

fn personal_ranks_by_effort_id(efforts: &[segment_efforts::Model], user_id: i32) -> HashMap<i32, i32> {
    rank_efforts_by_segment(
        efforts
            .iter()
            .filter(|effort| effort.user_id == user_id)
            .cloned()
            .collect(),
    )
}

fn personal_best_duration_by_segment(efforts: &[segment_efforts::Model], user_id: i32) -> HashMap<i32, i32> {
    efforts.iter().filter(|effort| effort.user_id == user_id).fold(
        HashMap::<i32, i32>::new(),
        |mut best_by_segment, effort| {
            best_by_segment
                .entry(effort.segment_id)
                .and_modify(|best| {
                    if effort.duration_seconds < *best {
                        *best = effort.duration_seconds;
                    }
                })
                .or_insert(effort.duration_seconds);

            best_by_segment
        },
    )
}

async fn load_activity_segment_efforts(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_id: i32,
) -> Result<Vec<ActivitySegmentEffort>, AppError> {
    let mut efforts_by_activity = load_activity_segment_efforts_by_activity_ids(db, user_id, &[activity_id]).await?;

    Ok(efforts_by_activity.remove(&activity_id).unwrap_or_default())
}

async fn load_activity_segment_efforts_by_activity_ids(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<HashMap<i32, Vec<ActivitySegmentEffort>>, AppError> {
    if activity_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let input = load_activity_segment_effort_input(db, user_id, activity_ids).await?;
    if input.efforts.is_empty() {
        return Ok(HashMap::new());
    }
    let rank_maps = activity_segment_effort_rank_maps(db, user_id, &input).await?;

    Ok(group_activity_segment_efforts(input, rank_maps))
}

struct ActivitySegmentEffortInput {
    efforts: Vec<segment_efforts::Model>,
    segments_by_id: HashMap<i32, segments::Model>,
    user_summary_by_segment: HashMap<i32, segment_user_summaries::Model>,
}

struct ActivitySegmentEffortRankMaps {
    overall_ranks_by_effort_id: HashMap<i32, i32>,
    personal_ranks_by_effort_id: HashMap<i32, i32>,
    personal_best_duration_by_segment: HashMap<i32, i32>,
}

async fn load_activity_segment_effort_input(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<ActivitySegmentEffortInput, AppError> {
    let efforts = segment_efforts::Model::list_by_user_activity_ids(db, user_id, activity_ids).await?;
    let segment_ids = unique_effort_segment_ids(&efforts);
    let user_summary_by_segment = load_segment_user_summaries_by_segment(db, user_id, &segment_ids).await?;
    let segments_by_id = load_segments_by_id(db, &segment_ids).await?;

    Ok(ActivitySegmentEffortInput {
        efforts,
        segments_by_id,
        user_summary_by_segment,
    })
}

fn unique_effort_segment_ids(efforts: &[segment_efforts::Model]) -> Vec<i32> {
    let mut segment_ids = efforts.iter().map(|effort| effort.segment_id).collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();
    segment_ids
}

async fn load_segment_user_summaries_by_segment(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    segment_ids: &[i32],
) -> Result<HashMap<i32, segment_user_summaries::Model>, AppError> {
    Ok(
        segment_user_summaries::Model::list_by_user_segment_ids(db, user_id, segment_ids)
            .await?
            .into_iter()
            .map(|summary| (summary.segment_id, summary))
            .collect(),
    )
}

async fn load_segments_by_id(
    db: &sea_orm::DatabaseConnection,
    segment_ids: &[i32],
) -> Result<HashMap<i32, segments::Model>, AppError> {
    Ok(segments::Model::list_by_ids(db, segment_ids)
        .await?
        .into_iter()
        .map(|segment| (segment.id, segment))
        .collect())
}

async fn activity_segment_effort_rank_maps(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    input: &ActivitySegmentEffortInput,
) -> Result<ActivitySegmentEffortRankMaps, AppError> {
    if can_use_cached_activity_segment_ranks(input) {
        return Ok(cached_activity_segment_effort_rank_maps(input));
    }

    let segment_ids = unique_effort_segment_ids(&input.efforts);
    let all_segment_efforts = segment_efforts::Model::list_by_segment_ids(db, &segment_ids).await?;

    Ok(ActivitySegmentEffortRankMaps {
        overall_ranks_by_effort_id: overall_ranks_by_effort_id(&all_segment_efforts),
        personal_ranks_by_effort_id: personal_ranks_by_effort_id(&all_segment_efforts, user_id),
        personal_best_duration_by_segment: personal_best_duration_by_segment(&all_segment_efforts, user_id),
    })
}

fn can_use_cached_activity_segment_ranks(input: &ActivitySegmentEffortInput) -> bool {
    input.efforts.iter().all(|effort| {
        let Some(segment) = input.segments_by_id.get(&effort.segment_id) else {
            return false;
        };
        let Some(user_summary) = input.user_summary_by_segment.get(&effort.segment_id) else {
            return false;
        };

        effort.overall_rank.is_some()
            && effort.user_rank.is_some()
            && effort.updated_at >= segment.last_activity_change_at
            && user_summary.updated_at >= segment.last_activity_change_at
    })
}

fn cached_activity_segment_effort_rank_maps(input: &ActivitySegmentEffortInput) -> ActivitySegmentEffortRankMaps {
    ActivitySegmentEffortRankMaps {
        overall_ranks_by_effort_id: input
            .efforts
            .iter()
            .filter_map(|effort| effort.overall_rank.map(|rank| (effort.id, rank)))
            .collect(),
        personal_ranks_by_effort_id: input
            .efforts
            .iter()
            .filter_map(|effort| effort.user_rank.map(|rank| (effort.id, rank)))
            .collect(),
        personal_best_duration_by_segment: input
            .user_summary_by_segment
            .iter()
            .filter_map(|(segment_id, summary)| {
                summary
                    .personal_best_duration_seconds
                    .map(|duration| (*segment_id, duration))
            })
            .collect(),
    }
}

fn group_activity_segment_efforts(
    input: ActivitySegmentEffortInput,
    rank_maps: ActivitySegmentEffortRankMaps,
) -> HashMap<i32, Vec<ActivitySegmentEffort>> {
    let mut efforts_by_activity = HashMap::<i32, Vec<ActivitySegmentEffort>>::new();

    for effort in input.efforts {
        let Some(segment_title) = input
            .segments_by_id
            .get(&effort.segment_id)
            .map(|segment| segment.title.clone())
        else {
            continue;
        };

        efforts_by_activity
            .entry(effort.activity_id)
            .or_default()
            .push(ActivitySegmentEffort {
                segment_id: effort.segment_id,
                segment_title,
                effort_index: effort.effort_index,
                duration_seconds: effort.duration_seconds,
                start_route_point_index: effort.start_route_point_index,
                end_route_point_index: effort.end_route_point_index,
                overall_rank: rank_maps.overall_ranks_by_effort_id.get(&effort.id).copied(),
                personal_rank: rank_maps.personal_ranks_by_effort_id.get(&effort.id).copied(),
                personal_best_duration_seconds: rank_maps
                    .personal_best_duration_by_segment
                    .get(&effort.segment_id)
                    .copied(),
            });
    }

    efforts_by_activity
}

async fn load_activity_achievement_highlights_by_activity_ids(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<HashMap<i32, Vec<ActivityAchievementHighlight>>, AppError> {
    if activity_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(
        activity_analytics::Model::list_by_user_activity_ids(db, user_id, activity_ids)
            .await?
            .into_iter()
            .map(|analytics| {
                (
                    analytics.activity_id,
                    analytics
                        .achievement_highlights_json
                        .map(|stored| stored.items)
                        .unwrap_or_default(),
                )
            })
            .collect(),
    )
}

#[utoipa::path(
    get,
    path = "/activities",
    params(ActivityListParams),
    responses(
        (status = 200, description = "Recent activities for the authenticated user", body = PaginatedResponse<ActivityResponse>, example = json!({"data": [{"id": 42,"title": "Saturday hill repeats","sport": "ride","source": "manual_upload","activity_type": "training","started_at": "2026-09-26T13:00:00Z","distance_meters": 25430.5}],"metadata": {"page": 1,"per_page": 20,"total": 1,"total_pages": 1}})),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
#[tracing::instrument(name = "activity.list", skip_all, fields(user_id = user.id))]
pub async fn list_activities(
    UserContext { user, .. }: UserContext<AppStorage>,
    Query(params): Query<ActivityListParams>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<PaginatedResponse<ActivityResponse>>, AppError> {
    let pagination = params.pagination();
    let activities = activities::Model::paginate_for_user(&state.db, user.id, &pagination, params.sport).await?;

    let activity_ids = activities.data.iter().map(|activity| activity.id).collect::<Vec<_>>();
    let mut achievement_highlights_by_activity =
        load_activity_achievement_highlights_by_activity_ids(&state.db, user.id, &activity_ids).await?;

    Ok(Json(activities.map(|activity| {
        let achievement_highlights = achievement_highlights_by_activity
            .remove(&activity.id)
            .unwrap_or_default();

        ActivityResponse::from_summary_with_achievement_highlights(activity, achievement_highlights)
    })))
}

#[utoipa::path(
    get,
    path = "/activities/{id}",
    params(
        ("id" = i32, Path, description = "Activity ID", example = 42)
    ),
    responses(
        (status = 200, description = "Activity detail", body = ActivityResponse, example = json!({"id": 42,"title": "Saturday hill repeats","sport": "ride","source": "manual_upload","activity_type": "training","original_filename": "saturday-hills.fit","format": "fit","started_at": "2026-09-26T13:00:00Z","ended_at": "2026-09-26T14:12:00Z","location": "Detroit, Michigan","distance_meters": 25430.5,"moving_time_seconds": 4200,"total_time_seconds": 4320,"elevation_gain_meters": 312.4,"average_speed_mps": 6.05,"average_heart_rate_bpm": 148,"estimated_ftp_watts": 245,"route_points": [{"elapsed_seconds": 900,"latitude": 42.3314,"longitude": -83.0458,"distance_meters": 4200.5,"elevation_meters": 183.2}],"can_regenerate": true,"can_download_source_file": true})),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
#[tracing::instrument(name = "activity.get", skip_all, fields(user_id = user.id, activity_id = id))]
pub async fn get_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<ActivityResponse>, AppError> {
    let activity = activities::Model::find_owned(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, activity.id).await?;
    let training_analysis = load_activity_training_analysis_by_activity_id(&state.db, activity.id).await?;
    let can_download_source_file = match activity.activity_import_id {
        Some(import_id) => activity_import_artifacts::Entity::downloadable_original(&state.db, user.id, import_id)
            .await?
            .is_some(),
        None => false,
    };
    let mut response = ActivityResponse::from_detail(activity, segment_efforts);
    response.training_analysis = training_analysis;
    response.can_download_source_file = can_download_source_file;

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/activities/{id}/source-file",
    params(
        ("id" = i32, Path, description = "Activity ID", example = 42)
    ),
    responses(
        (status = 200, description = "Original retained source file for the activity"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity source file not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn download_activity_source_file(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Response, AppError> {
    let activity = activities::Model::find_owned(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let activity_import_id = activity
        .activity_import_id
        .ok_or_else(|| AppError::not_found("Activity source file not found"))?;
    let source_artifact =
        activity_import_artifacts::Entity::downloadable_original(&state.db, user.id, activity_import_id)
            .await?
            .ok_or_else(|| AppError::not_found("Activity source file not found"))?;
    let source_path = resolve_activity_import_storage_path(&state.uploads_dir, &source_artifact.storage_path)?;
    let bytes = match tokio::fs::read(&source_path).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::not_found("Activity source file not found"));
        }
        Err(error) => return Err(error.into()),
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(activity_source_content_type(&source_artifact.format)),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .map_err(|_| AppError::internal("Failed to build source file response"))?,
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&content_disposition_header(&source_artifact.original_filename))
            .map_err(|_| AppError::internal("Failed to build source file response"))?,
    );

    Ok((headers, Body::from(bytes)).into_response())
}

#[utoipa::path(
    patch,
    path = "/activities/{id}",
    params(
        ("id" = i32, Path, description = "Activity ID", example = 42)
    ),
    request_body(content = UpdateActivityRequest, example = json!({"title": "Saturday hill repeats","activity_type": "training"})),
    responses(
        (status = 200, description = "Updated activity detail", body = ActivityResponse, example = json!({"id": 42,"title": "Saturday hill repeats","sport": "ride","source": "manual_upload","activity_type": "training","original_filename": "saturday-hills.fit","format": "fit","started_at": "2026-09-26T13:00:00Z","ended_at": "2026-09-26T14:12:00Z","location": "Detroit, Michigan","distance_meters": 25430.5,"moving_time_seconds": 4200,"total_time_seconds": 4320,"elevation_gain_meters": 312.4,"average_speed_mps": 6.05,"average_heart_rate_bpm": 148,"estimated_ftp_watts": 245,"route_points": [{"elapsed_seconds": 900,"latitude": 42.3314,"longitude": -83.0458,"distance_meters": 4200.5,"elevation_meters": 183.2}],"can_regenerate": true,"can_download_source_file": true})),
        (status = 400, description = "Invalid activity update", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<UpdateActivityRequest>,
) -> Result<Json<ActivityResponse>, AppError> {
    let activity = activities::Model::find_owned(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let updated = bike_core::activity_lifecycle::update_activity_details(
        &state.db,
        &state.tasks,
        activity,
        activities::ActivityDetailsUpdate {
            title: request.title,
            activity_type: request.activity_type,
            sport: request.sport,
            location: request.location,
            description: request.description,
            private_notes: request.private_notes,
            gear_name: request.gear_name,
        },
    )
    .await?;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, updated.id).await?;
    let training_analysis = load_activity_training_analysis_by_activity_id(&state.db, updated.id).await?;
    let mut response = ActivityResponse::from_detail(updated, segment_efforts);
    response.training_analysis = training_analysis;

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/activities/{id}",
    params(
        ("id" = i32, Path, description = "Activity ID", example = 42)
    ),
    responses(
        (status = 204, description = "Activity deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
#[tracing::instrument(name = "activity.delete", skip_all, fields(user_id = user.id, activity_id = id))]
pub async fn delete_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<StatusCode, AppError> {
    let activity = activities::Model::find_owned(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;

    let fitness_dirty_from_day = activity.started_at.date_naive();
    let affected_segment_ids =
        delete_activity_with_derived_state(&state.db, &state.uploads_dir, user.id, activity).await?;
    let changed_at = Utc::now();
    mark_user_fitness_dirty(&state.db, user.id, fitness_dirty_from_day, changed_at).await?;
    mark_segment_activity_changes(&state.db, &affected_segment_ids, changed_at).await?;
    state.tasks.rebuild_fitness_freshness(user.id).await?;
    state.tasks.rebuild_segment_analytics(affected_segment_ids).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/activities/{id}/regenerate",
    params(
        ("id" = i32, Path, description = "Activity ID", example = 42)
    ),
    responses(
        (status = 202, description = "Activity reprocessing started", body = bike_core::jobs::QueuedJobReference),
        (status = 400, description = "Activity cannot be regenerated", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn regenerate_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<(StatusCode, Json<bike_core::jobs::QueuedJobReference>), AppError> {
    let activity = activities::Model::find_owned(&state.db, id, user.id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    if activity.activity_import_id.is_none() {
        return Err(AppError::bad_request("Only uploaded activities can be regenerated"));
    }
    let job = state
        .tasks
        .reprocess_activity_import(activity.id)
        .await
        .map_err(AppError::internal)?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

fn activity_source_content_type(format: &str) -> &'static str {
    match format {
        "gpx" => "application/gpx+xml",
        "tcx" => "application/vnd.garmin.tcx+xml",
        "fit" => "application/octet-stream",
        _ => "application/octet-stream",
    }
}

fn content_disposition_header(filename: &str) -> String {
    format!(
        "attachment; filename=\"{}\"",
        safe_content_disposition_filename(filename)
    )
}

fn safe_content_disposition_filename(filename: &str) -> String {
    let safe = filename
        .chars()
        .map(|ch| match ch {
            '"' | '\\' | '/' => '_',
            ch if ch.is_ascii_control() || !ch.is_ascii() => '_',
            ch => ch,
        })
        .collect::<String>()
        .trim()
        .to_string();

    if safe.is_empty() {
        "activity-source-file".to_string()
    } else {
        safe
    }
}

fn resolve_activity_import_storage_path(uploads_dir: &str, storage_path: &str) -> Result<PathBuf, AppError> {
    let relative_path = FsPath::new(storage_path);

    if relative_path.is_absolute() {
        return Err(AppError::internal("Invalid activity source file path"));
    }

    for component in relative_path.components() {
        match component {
            Component::Normal(_) | Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::internal("Invalid activity source file path"));
            }
        }
    }

    Ok(FsPath::new(uploads_dir).join(relative_path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::activity_details::{serialize_derived_activity_data, StoredActivityDerivedData};
    use bike_core::activity_training_analysis::ActivityTrainingAnalysisResponse;
    use bike_core::platform::data::pagination::PaginatedResponse;
    use serde_json::json;

    #[test]
    fn edit_request_distinguishes_omitted_fields_and_explicit_text_clears() {
        let empty: UpdateActivityRequest = serde_json::from_value(json!({})).unwrap();
        assert!(empty.sport.is_none());
        assert!(empty.location.is_none());
        assert!(empty.description.is_none());
        let request: UpdateActivityRequest = serde_json::from_value(json!({
            "sport": "hike", "activity_type": "race", "location": "", "description": ""
        }))
        .unwrap();
        assert_eq!(request.sport, Some(ActivitySport::Hike));
        assert_eq!(request.activity_type, Some(ActivityType::Race));
        assert_eq!(request.location.as_deref(), Some(""));
        assert_eq!(request.description.as_deref(), Some(""));
        assert!(request.title.is_none());
        assert!(serde_json::from_value::<UpdateActivityRequest>(json!({"sport": "unknown_sport"})).is_err());
    }

    #[tokio::test]
    async fn regeneration_accepts_an_owned_retained_file_as_a_native_job() {
        use crate::controllers::test_support as http;
        use bike_core::test_support as fixtures;
        use std::sync::{Arc, Mutex};
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([
                vec![fixtures::reprocess_activity()],
                vec![fixtures::reprocess_activity()],
            ])
            .append_query_results([vec![fixtures::reprocess_import()]])
            .append_query_results([vec![fixtures::reprocess_artifact()]])
            .into_connection();
        let requests = Arc::new(Mutex::new(fixtures::temporal_client::Requests::default()));
        let state = http::temporal_state(db.clone(), requests.clone()).await;
        let response = http::local_user_request(
            state,
            http::json_request("POST", "/api/activities/42/regenerate", serde_json::json!({})),
            202,
        )
        .await;
        let requests = requests.lock().unwrap();
        assert_eq!(requests.starts.len(), 1);
        assert_eq!(response["id"], requests.starts[0].workflow_id);
        assert_eq!(response["status"], "running");
        assert_eq!(
            requests.starts[0].workflow_type.as_ref().unwrap().name,
            "reprocess_activity_import"
        );
        let log = format!("{:?}", db.into_transaction_log());
        assert!(!log.contains("INSERT"));
        assert!(!log.contains("background_tasks"));
    }

    #[test]
    fn feed_and_detail_keep_source_description_and_gear() {
        let mut model = activity_response_test_model(Utc::now());
        let metadata = bike_core::activity_data::ActivitySourceMetadata {
            location: None,
            private_notes: Some("Fixture private notes".into()),
            description: Some("Fixture description".into()),
            gear: Some(bike_core::activity_data::ActivityGear {
                id: "fixture-bike".into(),
                name: Some("Fixture bicycle".into()),
            }),
        };
        model.derived_data_json = Some(activity_response_derived_data_json().with_source_metadata(metadata.clone()));
        let summary = ActivityResponse::from_summary_with_achievement_highlights(model.clone(), vec![]);
        let detail = ActivityResponse::from_detail(model, vec![]);
        for response in [summary, detail] {
            assert_eq!(response.description, metadata.description);
            assert_eq!(response.private_notes, metadata.private_notes);
            assert_eq!(response.gear, metadata.gear);
        }
    }

    #[test]
    fn activity_response_maps_model_fields() {
        let now = Utc::now();
        let response = ActivityResponse::from_detail(
            activity_response_test_model(now),
            vec![activity_segment_effort_fixture()],
        );

        assert_eq!(response.id, 42);
        assert_eq!(response.title, "Evening Ride");
        assert_eq!(response.sport, "ride");
        assert_eq!(response.original_filename.as_deref(), Some("evening-ride.gpx"));
        assert_eq!(response.format.as_deref(), Some("gpx"));
        assert!(response.location.is_some());
        assert_eq!(response.max_heart_rate_bpm, Some(172));
        assert_eq!(response.laps.len(), 1);
        assert_eq!(response.chart_points.len(), 1);
        assert_eq!(response.route_points.len(), 1);
        assert_eq!(response.segment_efforts.len(), 1);
        assert_eq!(response.segment_efforts[0].start_route_point_index, 0);
        assert_eq!(response.segment_efforts[0].overall_rank, Some(1));
        assert_eq!(response.segment_efforts[0].personal_rank, Some(1));
        assert_eq!(response.segment_efforts[0].personal_best_duration_seconds, Some(312));
        assert_eq!(response.training_analysis, None);
        assert!(response.can_regenerate);
    }

    fn activity_response_test_model(now: DateTime<Utc>) -> activities::Model {
        activities::Model {
            id: 42,
            user_id: 8,
            activity_import_id: Some(9),
            title: "Evening Ride".to_string(),
            original_filename: Some("evening-ride.gpx".to_string()),
            format: Some("gpx".to_string()),
            started_at: now,
            ended_at: Some(now),
            distance_meters: Some(32100.0),
            moving_time_seconds: Some(3600),
            total_time_seconds: Some(3650),
            elevation_gain_meters: Some(420.0),
            elevation_loss_meters: Some(415.0),
            average_speed_mps: Some(8.91),
            max_speed_mps: Some(16.2),
            average_heart_rate_bpm: Some(138),
            max_heart_rate_bpm: Some(172),
            average_cadence_rpm: Some(86),
            max_cadence_rpm: Some(108),
            calories: Some(640),
            derived_data_json: Some(activity_response_derived_data_json()),
            created_at: now,
            updated_at: now,
            ..bike_core::test_support::activity()
        }
    }

    fn activity_response_derived_data_json() -> StoredActivityDerivedData {
        serialize_derived_activity_data(&ActivityDerivedData {
            recording: Default::default(),
            laps: vec![activity_response_lap_fixture()],
            chart_points: vec![activity_response_chart_point_fixture()],
            route_points: vec![activity_response_route_point_fixture()],
        })
        .expect("serialize derived activity data")
    }

    fn activity_response_lap_fixture() -> ActivityLap {
        ActivityLap {
            lap_index: 1,
            title: "Full activity".to_string(),
            start_offset_seconds: Some(0),
            duration_seconds: Some(3650),
            distance_meters: Some(32100.0),
            elevation_gain_meters: Some(420.0),
            elevation_loss_meters: Some(415.0),
            average_speed_mps: Some(8.91),
            max_speed_mps: Some(16.2),
            average_heart_rate_bpm: Some(138),
            max_heart_rate_bpm: Some(172),
            average_cadence_rpm: Some(86),
            max_cadence_rpm: Some(108),
            calories: Some(640),
        }
    }

    fn activity_response_chart_point_fixture() -> ActivityChartPoint {
        ActivityChartPoint {
            elapsed_seconds: 0,
            distance_meters: Some(0.0),
            elevation_meters: Some(100.0),
            speed_mps: None,
            heart_rate_bpm: Some(130),
            cadence_rpm: Some(82),
            power_watts: None,
        }
    }

    fn activity_response_route_point_fixture() -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds: 0,
            latitude: 45.0,
            longitude: -122.0,
            distance_meters: Some(0.0),
            elevation_meters: Some(100.0),
            speed_mps: Some(0.0),
            heart_rate_bpm: Some(130),
            cadence_rpm: Some(82),
            power_watts: None,
        }
    }

    fn activity_segment_effort_fixture() -> ActivitySegmentEffort {
        ActivitySegmentEffort {
            segment_id: 5,
            segment_title: "North Climb".to_string(),
            effort_index: 1,
            duration_seconds: 312,
            start_route_point_index: 0,
            end_route_point_index: 0,
            overall_rank: Some(1),
            personal_rank: Some(1),
            personal_best_duration_seconds: Some(312),
        }
    }

    #[test]
    fn activity_response_can_hold_training_analysis() {
        let now = Utc::now();
        let mut response = ActivityResponse::from_detail(
            activities::Model {
                id: 42,
                user_id: 8,
                activity_import_id: None,
                title: "Evening Ride".to_string(),
                sport: "ride".to_string(),
                source: "manual_upload".to_string(),
                original_filename: Some("evening-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: bike_core::activity_type::ActivityType::Training.as_str().to_string(),
                started_at: now,
                ended_at: Some(now),
                distance_meters: Some(32100.0),
                moving_time_seconds: Some(3600),
                total_time_seconds: Some(3650),
                elevation_gain_meters: Some(420.0),
                elevation_loss_meters: Some(415.0),
                average_speed_mps: Some(8.91),
                max_speed_mps: Some(16.2),
                average_heart_rate_bpm: Some(138),
                max_heart_rate_bpm: Some(172),
                average_cadence_rpm: Some(86),
                max_cadence_rpm: Some(108),
                calories: Some(640),
                created_at: now,
                updated_at: now,
                ..bike_core::test_support::activity()
            },
            Vec::new(),
        );

        response.training_analysis = Some(ActivityTrainingAnalysisResponse {
            ride_focus: bike_core::activity_training_analysis::ActivityRideFocus::MixedXc,
            route_family_key: Some("post-canyon".to_string()),
            comparable_distance_bucket_meters: Some(30_000),
            comparable_elevation_gain_bucket_meters: Some(400),
            aerobic_decoupling_percent: Some(4.8),
            z2_time_seconds: 120,
            z2_distance_meters: Some(1000.0),
            z2_average_speed_mps: Some(8.33),
            climbing_time_seconds: 300,
            climbing_elevation_gain_meters: Some(84.0),
            sustained_climb_count: 2,
        });

        assert_eq!(
            response.training_analysis.as_ref().map(|value| value.z2_time_seconds),
            Some(120)
        );
        assert_eq!(
            response
                .training_analysis
                .as_ref()
                .map(|value| value.sustained_climb_count),
            Some(2)
        );
    }

    #[test]
    fn source_file_response_helpers_are_header_safe() {
        assert_eq!(
            content_disposition_header("Morning Ride.fit"),
            "attachment; filename=\"Morning Ride.fit\""
        );
        assert_eq!(
            content_disposition_header("../ride\"bad.fit"),
            "attachment; filename=\".._ride_bad.fit\""
        );
        assert_eq!(activity_source_content_type("fit"), "application/octet-stream");
        assert_eq!(activity_source_content_type("tcx"), "application/vnd.garmin.tcx+xml");
    }

    #[test]
    fn source_file_path_resolution_rejects_unsafe_relative_paths() {
        assert_eq!(
            resolve_activity_import_storage_path("/uploads", "activity-imports/u/ride.fit").unwrap(),
            FsPath::new("/uploads").join("activity-imports/u/ride.fit")
        );
        assert!(resolve_activity_import_storage_path("/uploads", "../ride.fit").is_err());
        assert!(resolve_activity_import_storage_path("/uploads", "/tmp/ride.fit").is_err());
    }

    fn make_segment_effort_model(
        id: i32,
        user_id: i32,
        segment_id: i32,
        activity_id: i32,
        duration_seconds: i32,
    ) -> segment_efforts::Model {
        let now = Utc::now();

        segment_efforts::Model {
            id,
            user_id,
            segment_id,
            activity_id,
            effort_index: 1,
            start_route_point_index: 0,
            end_route_point_index: 10,
            start_elapsed_seconds: 0,
            end_elapsed_seconds: duration_seconds,
            duration_seconds,
            distance_meters: Some(1000.0),
            overall_rank: None,
            user_rank: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn ranks_segment_efforts_for_overall_and_personal_history() {
        let efforts = vec![
            make_segment_effort_model(1, 7, 11, 101, 320),
            make_segment_effort_model(2, 7, 11, 102, 300),
            make_segment_effort_model(3, 9, 11, 103, 290),
            make_segment_effort_model(4, 7, 12, 201, 410),
            make_segment_effort_model(5, 7, 12, 202, 390),
        ];

        let overall = overall_ranks_by_effort_id(&efforts);
        let personal = personal_ranks_by_effort_id(&efforts, 7);

        assert_eq!(overall.get(&3), Some(&1));
        assert_eq!(overall.get(&2), Some(&2));
        assert_eq!(overall.get(&1), Some(&3));
        assert_eq!(personal.get(&2), Some(&1));
        assert_eq!(personal.get(&1), Some(&2));
        assert_eq!(personal.get(&5), Some(&1));
        assert_eq!(personal.get(&4), Some(&2));
        assert_eq!(personal.get(&3), None);
    }

    #[test]
    fn loads_personal_best_duration_by_segment() {
        let efforts = vec![
            make_segment_effort_model(1, 7, 11, 101, 320),
            make_segment_effort_model(2, 7, 11, 102, 300),
            make_segment_effort_model(3, 9, 11, 103, 290),
            make_segment_effort_model(4, 7, 12, 201, 410),
            make_segment_effort_model(5, 7, 12, 202, 390),
        ];

        let personal_best = personal_best_duration_by_segment(&efforts, 7);

        assert_eq!(personal_best.get(&11), Some(&300));
        assert_eq!(personal_best.get(&12), Some(&390));
        assert_eq!(personal_best.get(&13), None);
    }

    #[test]
    fn paginated_activity_response_maps_models() {
        let now = Utc::now();
        let response = PaginatedResponse::new(
            vec![activities::Model {
                id: 7,
                user_id: 8,
                activity_import_id: Some(9),
                title: "Morning Ride".to_string(),
                sport: "ride".to_string(),
                source: "manual_upload".to_string(),
                original_filename: Some("morning-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: bike_core::activity_type::ActivityType::Training.as_str().to_string(),
                started_at: now,
                ended_at: Some(now),
                distance_meters: Some(40200.0),
                moving_time_seconds: Some(3600),
                total_time_seconds: Some(3900),
                elevation_gain_meters: Some(520.0),
                elevation_loss_meters: Some(515.0),
                average_speed_mps: Some(9.5),
                max_speed_mps: Some(16.2),
                average_heart_rate_bpm: Some(142),
                max_heart_rate_bpm: Some(171),
                average_cadence_rpm: Some(86),
                max_cadence_rpm: Some(104),
                calories: Some(860),
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
                        recording: Default::default(),
                        laps: Vec::new(),
                        chart_points: Vec::new(),
                        route_points: vec![ActivityRoutePoint {
                            elapsed_seconds: 0,
                            latitude: 45.0,
                            longitude: -122.0,
                            distance_meters: Some(0.0),
                            elevation_meters: Some(100.0),
                            speed_mps: Some(0.0),
                            heart_rate_bpm: Some(130),
                            cadence_rpm: Some(82),
                            power_watts: None,
                        }],
                    })
                    .expect("serialize derived activity data"),
                ),
                created_at: now,
                updated_at: now,
                ..bike_core::test_support::activity()
            }],
            2,
            10,
            24,
        )
        .map(|model| ActivityResponse::from_summary_with_achievement_highlights(model, Vec::new()));

        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].title, "Morning Ride");
        assert!(response.data[0].location.is_some());
        assert_eq!(response.metadata.page, 2);
        assert_eq!(response.metadata.per_page, 10);
        assert_eq!(response.metadata.total, 24);
        assert_eq!(response.metadata.total_pages, 3);
    }

    #[test]
    fn summary_response_downsamples_route_points_for_stream_preview() {
        let now = Utc::now();
        let route_points = (0..40)
            .map(|index| ActivityRoutePoint {
                elapsed_seconds: index,
                latitude: 45.0 + (index as f64 * 0.001),
                longitude: -122.0 + (index as f64 * 0.001),
                distance_meters: Some(index as f64 * 100.0),
                elevation_meters: Some(100.0 + index as f64),
                speed_mps: Some(5.0 + index as f64 * 0.1),
                heart_rate_bpm: Some(120 + index),
                cadence_rpm: Some(80),
                power_watts: None,
            })
            .collect::<Vec<_>>();
        let response = ActivityResponse::from_summary_with_achievement_highlights(
            activities::Model {
                id: 7,
                user_id: 8,
                activity_import_id: Some(9),
                title: "Morning Ride".to_string(),
                sport: "ride".to_string(),
                source: "manual_upload".to_string(),
                original_filename: Some("morning-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: bike_core::activity_type::ActivityType::Training.as_str().to_string(),
                started_at: now,
                ended_at: Some(now),
                distance_meters: Some(40200.0),
                moving_time_seconds: Some(3600),
                total_time_seconds: Some(3900),
                elevation_gain_meters: Some(520.0),
                elevation_loss_meters: Some(515.0),
                average_speed_mps: Some(9.5),
                max_speed_mps: Some(16.2),
                average_heart_rate_bpm: Some(142),
                max_heart_rate_bpm: Some(171),
                average_cadence_rpm: Some(86),
                max_cadence_rpm: Some(104),
                calories: Some(860),
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
                        recording: Default::default(),
                        laps: Vec::new(),
                        chart_points: Vec::new(),
                        route_points,
                    })
                    .expect("serialize derived activity data"),
                ),
                created_at: now,
                updated_at: now,
                ..bike_core::test_support::activity()
            },
            Vec::new(),
        );

        assert_eq!(response.route_points.len(), MAX_ACTIVITY_STREAM_ROUTE_POINTS);
        assert_eq!(
            response.route_points.first().map(|point| point.elapsed_seconds),
            Some(0)
        );
        assert_eq!(
            response.route_points.last().map(|point| point.elapsed_seconds),
            Some(39)
        );
    }

    #[test]
    fn summary_response_preserves_route_extrema_for_stream_preview() {
        let now = Utc::now();
        let mut route_points = (0..40)
            .map(|index| ActivityRoutePoint {
                elapsed_seconds: index,
                latitude: 45.0 + (index as f64 * 0.001),
                longitude: -122.0 + (index as f64 * 0.001),
                distance_meters: Some(index as f64 * 100.0),
                elevation_meters: Some(100.0 + index as f64),
                speed_mps: Some(5.0 + index as f64 * 0.1),
                heart_rate_bpm: Some(120 + index),
                cadence_rpm: Some(80),
                power_watts: None,
            })
            .collect::<Vec<_>>();

        route_points[7].latitude = 46.5;
        route_points[11].latitude = 44.2;
        route_points[13].longitude = -123.4;
        route_points[29].longitude = -120.8;

        let response = ActivityResponse::from_summary_with_achievement_highlights(
            activities::Model {
                id: 7,
                user_id: 8,
                activity_import_id: Some(9),
                title: "Morning Ride".to_string(),
                sport: "ride".to_string(),
                source: "manual_upload".to_string(),
                original_filename: Some("morning-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: bike_core::activity_type::ActivityType::Training.as_str().to_string(),
                started_at: now,
                ended_at: Some(now),
                distance_meters: Some(40200.0),
                moving_time_seconds: Some(3600),
                total_time_seconds: Some(3900),
                elevation_gain_meters: Some(520.0),
                elevation_loss_meters: Some(515.0),
                average_speed_mps: Some(9.5),
                max_speed_mps: Some(16.2),
                average_heart_rate_bpm: Some(142),
                max_heart_rate_bpm: Some(171),
                average_cadence_rpm: Some(86),
                max_cadence_rpm: Some(104),
                calories: Some(860),
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
                        recording: Default::default(),
                        laps: Vec::new(),
                        chart_points: Vec::new(),
                        route_points,
                    })
                    .expect("serialize derived activity data"),
                ),
                created_at: now,
                updated_at: now,
                ..bike_core::test_support::activity()
            },
            Vec::new(),
        );

        let sampled_elapsed_seconds = response
            .route_points
            .iter()
            .map(|point| point.elapsed_seconds)
            .collect::<Vec<_>>();

        assert!(sampled_elapsed_seconds.contains(&7));
        assert!(sampled_elapsed_seconds.contains(&11));
        assert!(sampled_elapsed_seconds.contains(&13));
        assert!(sampled_elapsed_seconds.contains(&29));
    }
}
