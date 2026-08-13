use crate::activity_analytics::ActivityAchievementHighlight;
use crate::activity_details::{
    deserialize_derived_activity_data, ActivityChartPoint, ActivityDerivedData, ActivityLap,
    ActivityRoutePoint,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, mark_activity_imports_processed, reprocess_activity_from_import,
};
use crate::activity_lifecycle::delete_activity_with_derived_state;
use crate::activity_location::location_from_derived_json;
use crate::activity_training_analysis::{
    load_activity_training_analysis_by_activity_id, ActivityTrainingAnalysisResponse,
};
use crate::activity_type::ActivityType;
use crate::analytics::{
    estimated_training_load, mark_segment_activity_changes, mark_user_fitness_dirty,
};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{
    activities, activity_analytics, activity_imports, segment_efforts, segment_user_summaries,
    segments,
};
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_activity_heart_rate_zones, ActivityHeartRateZoneSummary,
};
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::UserContext;
use kaleido::glass::data::pagination::{Paginatable, PaginatedResponse, PaginationParams};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::path::{Component, Path as FsPath, PathBuf};
use std::sync::Arc;
use utoipa::ToSchema;

const MAX_ACTIVITY_STREAM_ROUTE_POINTS: usize = 24;

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityResponse {
    pub id: i32,
    pub title: String,
    pub sport: String,
    pub source: String,
    pub activity_type: ActivityType,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub location: Option<String>,
    pub distance_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
    pub relative_effort: Option<i32>,
    pub estimated_ftp_watts: Option<i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heart_rate_zones: Vec<ActivityHeartRateZoneSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<ActivityRoutePoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub achievement_highlights: Vec<ActivityAchievementHighlight>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segment_efforts: Vec<ActivitySegmentEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub training_analysis: Option<ActivityTrainingAnalysisResponse>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_regenerate: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_download_source_file: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateActivityRequest {
    pub title: Option<String>,
    pub activity_type: Option<ActivityType>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivitySegmentEffort {
    pub segment_id: i32,
    pub segment_title: String,
    pub effort_index: i32,
    pub duration_seconds: i32,
    pub start_route_point_index: i32,
    pub end_route_point_index: i32,
    pub overall_rank: Option<i32>,
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

        Self::from_model(
            model,
            derived_data,
            location,
            achievement_highlights,
            Vec::new(),
            false,
        )
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

        Self {
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
            heart_rate_zones: deserialize_activity_heart_rate_zones(
                model.heart_rate_zones_json.as_ref(),
            ),
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

fn summary_derived_data(
    raw: Option<&crate::activity_details::StoredActivityDerivedData>,
) -> ActivityDerivedData {
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

        let point_index =
            sample_index * last_index / (MAX_ACTIVITY_STREAM_ROUTE_POINTS.saturating_sub(1));
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

fn personal_ranks_by_effort_id(
    efforts: &[segment_efforts::Model],
    user_id: i32,
) -> HashMap<i32, i32> {
    rank_efforts_by_segment(
        efforts
            .iter()
            .filter(|effort| effort.user_id == user_id)
            .cloned()
            .collect(),
    )
}

fn personal_best_duration_by_segment(
    efforts: &[segment_efforts::Model],
    user_id: i32,
) -> HashMap<i32, i32> {
    efforts
        .iter()
        .filter(|effort| effort.user_id == user_id)
        .fold(HashMap::<i32, i32>::new(), |mut best_by_segment, effort| {
            best_by_segment
                .entry(effort.segment_id)
                .and_modify(|best| {
                    if effort.duration_seconds < *best {
                        *best = effort.duration_seconds;
                    }
                })
                .or_insert(effort.duration_seconds);

            best_by_segment
        })
}

async fn load_activity_segment_efforts(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_id: i32,
) -> Result<Vec<ActivitySegmentEffort>, AppError> {
    let mut efforts_by_activity =
        load_activity_segment_efforts_by_activity_ids(db, user_id, &[activity_id]).await?;

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

    let effort_models = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::UserId.eq(user_id))
        .filter(segment_efforts::Column::ActivityId.is_in(activity_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::ActivityId)
        .order_by_asc(segment_efforts::Column::StartRoutePointIndex)
        .order_by_asc(segment_efforts::Column::EndRoutePointIndex)
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
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

    let user_summary_by_segment = segment_user_summaries::Entity::find()
        .filter(segment_user_summaries::Column::UserId.eq(user_id))
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|summary| (summary.segment_id, summary))
        .collect::<HashMap<_, _>>();
    let segments_by_id = segments::Entity::find()
        .filter(segments::Column::Id.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|segment| (segment.id, segment))
        .collect::<HashMap<_, _>>();
    let can_use_cached_ranks = effort_models.iter().all(|effort| {
        let Some(segment) = segments_by_id.get(&effort.segment_id) else {
            return false;
        };
        let Some(user_summary) = user_summary_by_segment.get(&effort.segment_id) else {
            return false;
        };

        effort.overall_rank.is_some()
            && effort.user_rank.is_some()
            && effort.updated_at >= segment.last_activity_change_at
            && user_summary.updated_at >= segment.last_activity_change_at
    });

    let (
        overall_ranks_by_effort_id,
        personal_ranks_by_effort_id,
        personal_best_duration_by_segment,
    ) = if can_use_cached_ranks {
        (
            effort_models
                .iter()
                .filter_map(|effort| effort.overall_rank.map(|rank| (effort.id, rank)))
                .collect::<HashMap<_, _>>(),
            effort_models
                .iter()
                .filter_map(|effort| effort.user_rank.map(|rank| (effort.id, rank)))
                .collect::<HashMap<_, _>>(),
            user_summary_by_segment
                .into_iter()
                .filter_map(|(segment_id, summary)| {
                    summary
                        .personal_best_duration_seconds
                        .map(|duration| (segment_id, duration))
                })
                .collect::<HashMap<_, _>>(),
        )
    } else {
        let all_segment_efforts = segment_efforts::Entity::find()
            .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
            .all(db)
            .await?;

        (
            overall_ranks_by_effort_id(&all_segment_efforts),
            personal_ranks_by_effort_id(&all_segment_efforts, user_id),
            personal_best_duration_by_segment(&all_segment_efforts, user_id),
        )
    };

    let mut efforts_by_activity = HashMap::<i32, Vec<ActivitySegmentEffort>>::new();

    for effort in effort_models {
        let Some(segment_title) = segments_by_id
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
                overall_rank: overall_ranks_by_effort_id.get(&effort.id).copied(),
                personal_rank: personal_ranks_by_effort_id.get(&effort.id).copied(),
                personal_best_duration_seconds: personal_best_duration_by_segment
                    .get(&effort.segment_id)
                    .copied(),
            });
    }

    Ok(efforts_by_activity)
}

async fn load_activity_achievement_highlights_by_activity_ids(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<HashMap<i32, Vec<ActivityAchievementHighlight>>, AppError> {
    if activity_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(activity_analytics::Entity::find()
        .filter(activity_analytics::Column::UserId.eq(user_id))
        .filter(activity_analytics::Column::ActivityId.is_in(activity_ids.iter().copied()))
        .all(db)
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
        .collect())
}

#[utoipa::path(
    get,
    path = "/api/activities",
    params(PaginationParams),
    responses(
        (status = 200, description = "Recent activities for the authenticated user", body = PaginatedResponse<ActivityResponse>),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_activities(
    UserContext { user, .. }: UserContext<AppStorage>,
    Query(params): Query<PaginationParams>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<PaginatedResponse<ActivityResponse>>, AppError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user.id))
        .order_by_desc(activities::Column::StartedAt)
        .order_by_desc(activities::Column::Id)
        .fetch_paginated(&state.db, &params)
        .await?;

    let activity_ids = activities
        .data
        .iter()
        .map(|activity| activity.id)
        .collect::<Vec<_>>();
    let mut achievement_highlights_by_activity =
        load_activity_achievement_highlights_by_activity_ids(&state.db, user.id, &activity_ids)
            .await?;

    Ok(Json(activities.map(|activity| {
        let achievement_highlights = achievement_highlights_by_activity
            .remove(&activity.id)
            .unwrap_or_default();

        ActivityResponse::from_summary_with_achievement_highlights(activity, achievement_highlights)
    })))
}

#[utoipa::path(
    get,
    path = "/api/activities/{id}",
    params(
        ("id" = i32, Path, description = "Activity ID")
    ),
    responses(
        (status = 200, description = "Activity detail", body = ActivityResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activities",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<ActivityResponse>, AppError> {
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, activity.id).await?;
    let training_analysis =
        load_activity_training_analysis_by_activity_id(&state.db, activity.id).await?;
    let mut response = ActivityResponse::from_detail(activity, segment_efforts);
    response.training_analysis = training_analysis;

    Ok(Json(response))
}

#[utoipa::path(
    get,
    path = "/api/activities/{id}/source-file",
    params(
        ("id" = i32, Path, description = "Activity ID")
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
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let activity_import_id = activity
        .activity_import_id
        .ok_or_else(|| AppError::not_found("Activity source file not found"))?;
    let activity_import = activity_imports::Entity::find()
        .filter(activity_imports::Column::Id.eq(activity_import_id))
        .filter(activity_imports::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity source file not found"))?;
    let source_path =
        resolve_activity_import_storage_path(&state.uploads_dir, &activity_import.storage_path)?;
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
        HeaderValue::from_static(activity_source_content_type(&activity_import.format)),
    );
    headers.insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .map_err(|_| AppError::internal("Failed to build source file response"))?,
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&content_disposition_header(
            &activity_import.original_filename,
        ))
        .map_err(|_| AppError::internal("Failed to build source file response"))?,
    );

    Ok((headers, Body::from(bytes)).into_response())
}

#[utoipa::path(
    patch,
    path = "/api/activities/{id}",
    params(
        ("id" = i32, Path, description = "Activity ID")
    ),
    request_body = UpdateActivityRequest,
    responses(
        (status = 200, description = "Updated activity detail", body = ActivityResponse),
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
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let mut active_model: activities::ActiveModel = activity.into();

    if let Some(activity_type) = request.activity_type {
        active_model.activity_type = Set(activity_type.as_str().to_string());
    }
    if let Some(title) = request.title {
        active_model.title = Set(normalize_activity_title(&title)?);
    }

    let updated = active_model.update(&state.db).await?;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, updated.id).await?;
    let training_analysis =
        load_activity_training_analysis_by_activity_id(&state.db, updated.id).await?;
    let mut response = ActivityResponse::from_detail(updated, segment_efforts);
    response.training_analysis = training_analysis;

    Ok(Json(response))
}

#[utoipa::path(
    delete,
    path = "/api/activities/{id}",
    params(
        ("id" = i32, Path, description = "Activity ID")
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
pub async fn delete_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<StatusCode, AppError> {
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;

    let fitness_dirty_from_day = activity.started_at.date_naive();
    let affected_segment_ids =
        delete_activity_with_derived_state(&state.db, &state.uploads_dir, user.id, activity)
            .await?;
    let changed_at = Utc::now();
    mark_user_fitness_dirty(&state.db, user.id, fitness_dirty_from_day, changed_at).await?;
    mark_segment_activity_changes(&state.db, &affected_segment_ids, changed_at).await?;
    state.tasks.rebuild_fitness_freshness(user.id).await;
    state
        .tasks
        .rebuild_segment_analytics(affected_segment_ids)
        .await;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/activities/{id}/regenerate",
    params(
        ("id" = i32, Path, description = "Activity ID")
    ),
    responses(
        (status = 200, description = "Regenerated activity detail", body = ActivityResponse),
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
) -> Result<Json<ActivityResponse>, AppError> {
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let activity_import_id = activity
        .activity_import_id
        .ok_or_else(|| AppError::bad_request("Only uploaded activities can be regenerated"))?;
    let activity_import = activity_imports::Entity::find()
        .filter(activity_imports::Column::Id.eq(activity_import_id))
        .filter(activity_imports::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity import not found"))?;
    let reprocessed = reprocess_activity_from_import(
        &state.db,
        &state.uploads_dir,
        user.id,
        activity,
        activity_import,
        None,
    )
    .await?;
    finalize_activity_import_batch(
        &state.db,
        &state.tasks,
        user.id,
        reprocessed.affected_segment_ids,
        Some(reprocessed.fitness_dirty_from_day),
        Utc::now(),
    )
    .await?;
    mark_activity_imports_processed(&state.db, &[activity_import_id]).await?;
    let updated = reprocessed.activity;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, updated.id).await?;

    Ok(Json(ActivityResponse::from_detail(
        updated,
        segment_efforts,
    )))
}

fn normalize_activity_title(raw_title: &str) -> Result<String, AppError> {
    let title = raw_title.trim();

    if title.is_empty() {
        return Err(AppError::validation_field(
            "title",
            "Activity title is required",
        ));
    }

    Ok(title.to_string())
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

fn resolve_activity_import_storage_path(
    uploads_dir: &str,
    storage_path: &str,
) -> Result<PathBuf, AppError> {
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
    use crate::activity_details::serialize_derived_activity_data;
    use crate::activity_training_analysis::ActivityTrainingAnalysisResponse;
    use kaleido::glass::data::pagination::PaginatedResponse;

    #[test]
    fn activity_response_maps_model_fields() {
        let now = Utc::now();
        let response = ActivityResponse::from_detail(
            activities::Model {
                id: 42,
                user_id: 8,
                activity_import_id: Some(9),
                title: "Evening Ride".to_string(),
                sport: "ride".to_string(),
                source: "manual_upload".to_string(),
                source_correlation_id: None,
                original_filename: Some("evening-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: crate::activity_type::ActivityType::Training
                    .as_str()
                    .to_string(),
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
                estimated_ftp_watts: None,
                heart_rate_zones_json: None,
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
                        laps: vec![ActivityLap {
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
                        }],
                        chart_points: vec![ActivityChartPoint {
                            elapsed_seconds: 0,
                            distance_meters: Some(0.0),
                            elevation_meters: Some(100.0),
                            speed_mps: None,
                            heart_rate_bpm: Some(130),
                            cadence_rpm: Some(82),
                            power_watts: None,
                        }],
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
            },
            vec![ActivitySegmentEffort {
                segment_id: 5,
                segment_title: "North Climb".to_string(),
                effort_index: 1,
                duration_seconds: 312,
                start_route_point_index: 0,
                end_route_point_index: 0,
                overall_rank: Some(1),
                personal_rank: Some(1),
                personal_best_duration_seconds: Some(312),
            }],
        );

        assert_eq!(response.id, 42);
        assert_eq!(response.title, "Evening Ride");
        assert_eq!(response.sport, "ride");
        assert_eq!(
            response.original_filename.as_deref(),
            Some("evening-ride.gpx")
        );
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
        assert_eq!(
            response.segment_efforts[0].personal_best_duration_seconds,
            Some(312)
        );
        assert_eq!(response.training_analysis, None);
        assert!(response.can_regenerate);
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
                source_correlation_id: None,
                original_filename: Some("evening-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: crate::activity_type::ActivityType::Training
                    .as_str()
                    .to_string(),
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
                estimated_ftp_watts: None,
                heart_rate_zones_json: None,
                derived_data_json: None,
                created_at: now,
                updated_at: now,
            },
            Vec::new(),
        );

        response.training_analysis = Some(ActivityTrainingAnalysisResponse {
            ride_focus: crate::activity_training_analysis::ActivityRideFocus::MixedXc,
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
            response
                .training_analysis
                .as_ref()
                .map(|value| value.z2_time_seconds),
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
    fn normalize_activity_title_trims_whitespace() {
        assert_eq!(
            normalize_activity_title("  Lunch Loop  ").unwrap(),
            "Lunch Loop"
        );
    }

    #[test]
    fn normalize_activity_title_rejects_blank_titles() {
        let error = normalize_activity_title("   ").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
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
        assert_eq!(
            activity_source_content_type("fit"),
            "application/octet-stream"
        );
        assert_eq!(
            activity_source_content_type("tcx"),
            "application/vnd.garmin.tcx+xml"
        );
    }

    #[test]
    fn source_file_path_resolution_rejects_unsafe_relative_paths() {
        assert_eq!(
            resolve_activity_import_storage_path("/uploads", "activity-imports/u/ride.fit")
                .unwrap(),
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
                source_correlation_id: None,
                original_filename: Some("morning-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: crate::activity_type::ActivityType::Training
                    .as_str()
                    .to_string(),
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
                estimated_ftp_watts: None,
                heart_rate_zones_json: None,
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
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
                source_correlation_id: None,
                original_filename: Some("morning-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: crate::activity_type::ActivityType::Training
                    .as_str()
                    .to_string(),
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
                estimated_ftp_watts: None,
                heart_rate_zones_json: None,
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
                        laps: Vec::new(),
                        chart_points: Vec::new(),
                        route_points,
                    })
                    .expect("serialize derived activity data"),
                ),
                created_at: now,
                updated_at: now,
            },
            Vec::new(),
        );

        assert_eq!(
            response.route_points.len(),
            MAX_ACTIVITY_STREAM_ROUTE_POINTS
        );
        assert_eq!(
            response
                .route_points
                .first()
                .map(|point| point.elapsed_seconds),
            Some(0)
        );
        assert_eq!(
            response
                .route_points
                .last()
                .map(|point| point.elapsed_seconds),
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
                source_correlation_id: None,
                original_filename: Some("morning-ride.gpx".to_string()),
                format: Some("gpx".to_string()),
                activity_type: crate::activity_type::ActivityType::Training
                    .as_str()
                    .to_string(),
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
                estimated_ftp_watts: None,
                heart_rate_zones_json: None,
                derived_data_json: Some(
                    serialize_derived_activity_data(&ActivityDerivedData {
                        laps: Vec::new(),
                        chart_points: Vec::new(),
                        route_points,
                    })
                    .expect("serialize derived activity data"),
                ),
                created_at: now,
                updated_at: now,
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
