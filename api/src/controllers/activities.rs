use crate::activity_details::{
    deserialize_derived_activity_data, ActivityChartPoint, ActivityDerivedData, ActivityLap,
    ActivityRoutePoint,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, reprocess_activity_from_import,
};
use crate::activity_lifecycle::delete_activity_with_derived_state;
use crate::activity_location::location_from_derived_json;
use crate::analytics::{mark_segment_activity_changes, mark_user_activity_change};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{
    activities, activity_imports, segment_efforts, segment_user_summaries, segments,
};
use crate::storage::AppStorage;
use crate::training_profile::{
    deserialize_activity_heart_rate_zones, ActivityHeartRateZoneSummary,
};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::UserContext;
use kaleido::glass::data::pagination::{Paginatable, PaginatedResponse, PaginationParams};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use utoipa::ToSchema;

const MAX_ACTIVITY_STREAM_ROUTE_POINTS: usize = 24;

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityResponse {
    pub id: i32,
    pub title: String,
    pub sport: String,
    pub source: String,
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
    pub segment_efforts: Vec<ActivitySegmentEffort>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub can_regenerate: bool,
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
    fn from_summary(model: activities::Model) -> Self {
        let derived_data = summary_derived_data(model.derived_data_json.as_ref());
        let location = location_from_derived_json(model.derived_data_json.as_ref());

        Self::from_model(model, derived_data, location, Vec::new(), false)
    }

    fn from_detail(model: activities::Model, segment_efforts: Vec<ActivitySegmentEffort>) -> Self {
        let can_regenerate = model.activity_import_id.is_some();
        let derived_data = deserialize_derived_activity_data(model.derived_data_json.as_ref());
        let location = location_from_derived_json(model.derived_data_json.as_ref());

        Self::from_model(
            model,
            derived_data,
            location,
            segment_efforts,
            can_regenerate,
        )
    }

    fn from_model(
        model: activities::Model,
        derived_data: ActivityDerivedData,
        location: Option<String>,
        segment_efforts: Vec<ActivitySegmentEffort>,
        can_regenerate: bool,
    ) -> Self {
        Self {
            id: model.id,
            title: model.title,
            sport: model.sport,
            source: model.source,
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
            estimated_ftp_watts: model.estimated_ftp_watts,
            heart_rate_zones: deserialize_activity_heart_rate_zones(
                model.heart_rate_zones_json.as_ref(),
            ),
            laps: derived_data.laps,
            chart_points: derived_data.chart_points,
            route_points: derived_data.route_points,
            segment_efforts,
            can_regenerate,
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
    let effort_models = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::UserId.eq(user_id))
        .filter(segment_efforts::Column::ActivityId.eq(activity_id))
        .order_by_asc(segment_efforts::Column::StartRoutePointIndex)
        .order_by_asc(segment_efforts::Column::EndRoutePointIndex)
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;
    if effort_models.is_empty() {
        return Ok(Vec::new());
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

    Ok(effort_models
        .into_iter()
        .filter_map(|effort| {
            let segment_title = segments_by_id.get(&effort.segment_id)?.title.clone();

            Some(ActivitySegmentEffort {
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
            })
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

    Ok(Json(activities.map(ActivityResponse::from_summary)))
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

    Ok(Json(ActivityResponse::from_detail(
        activity,
        segment_efforts,
    )))
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

    let affected_segment_ids =
        delete_activity_with_derived_state(&state.db, &state.uploads_dir, user.id, activity)
            .await?;
    let changed_at = Utc::now();
    mark_user_activity_change(&state.db, user.id, changed_at).await?;
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
        Utc::now(),
    )
    .await?;
    let updated = reprocessed.activity;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, updated.id).await?;

    Ok(Json(ActivityResponse::from_detail(
        updated,
        segment_efforts,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_details::serialize_derived_activity_data;
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
        assert!(response.can_regenerate);
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
        .map(ActivityResponse::from_summary);

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
            })
            .collect::<Vec<_>>();
        let response = ActivityResponse::from_summary(activities::Model {
            id: 7,
            user_id: 8,
            activity_import_id: Some(9),
            title: "Morning Ride".to_string(),
            sport: "ride".to_string(),
            source: "manual_upload".to_string(),
            source_correlation_id: None,
            original_filename: Some("morning-ride.gpx".to_string()),
            format: Some("gpx".to_string()),
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
        });

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
            })
            .collect::<Vec<_>>();

        route_points[7].latitude = 46.5;
        route_points[11].latitude = 44.2;
        route_points[13].longitude = -123.4;
        route_points[29].longitude = -120.8;

        let response = ActivityResponse::from_summary(activities::Model {
            id: 7,
            user_id: 8,
            activity_import_id: Some(9),
            title: "Morning Ride".to_string(),
            sport: "ride".to_string(),
            source: "manual_upload".to_string(),
            source_correlation_id: None,
            original_filename: Some("morning-ride.gpx".to_string()),
            format: Some("gpx".to_string()),
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
        });

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
