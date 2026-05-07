use crate::activity_details::{
    deserialize_derived_activity_data, derive_activity_detail_data,
    serialize_derived_activity_data, ActivityChartPoint, ActivityDerivedData,
    ActivityLap, ActivityRoutePoint,
};
use crate::activity_lifecycle::{
    delete_activity_with_derived_state, refresh_activity_derived_state,
};
use crate::activity_location::location_from_derived_json;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::activity_summary::summarize_activity_upload;
use crate::entities::{activities, activity_imports, segment_efforts, segments};
use crate::storage::AppStorage;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::UserContext;
use kaleido::glass::data::pagination::{PaginatedResponse, Paginatable, PaginationParams};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set,
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path as FsPath;
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
}

fn is_false(value: &bool) -> bool {
    !*value
}

impl ActivityResponse {
    fn from_summary(model: activities::Model) -> Self {
        let derived_data = summary_derived_data(model.derived_data_json.as_deref());
        let location = location_from_derived_json(model.derived_data_json.as_deref());

        Self::from_model(
            model,
            derived_data,
            location,
            Vec::new(),
            false,
        )
    }

    fn from_detail(model: activities::Model, segment_efforts: Vec<ActivitySegmentEffort>) -> Self {
        let can_regenerate = model.activity_import_id.is_some();
        let derived_data = deserialize_derived_activity_data(model.derived_data_json.as_deref());
        let location = location_from_derived_json(model.derived_data_json.as_deref());

        Self::from_model(model, derived_data, location, segment_efforts, can_regenerate)
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
            laps: derived_data.laps,
            chart_points: derived_data.chart_points,
            route_points: derived_data.route_points,
            segment_efforts,
            can_regenerate,
        }
    }
}

fn summary_derived_data(raw: Option<&str>) -> ActivityDerivedData {
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

    (0..MAX_ACTIVITY_STREAM_ROUTE_POINTS)
        .map(|index| {
            let point_index =
                index * (route_points.len().saturating_sub(1))
                    / (MAX_ACTIVITY_STREAM_ROUTE_POINTS.saturating_sub(1));
            route_points[point_index].clone()
        })
        .collect()
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
    let segment_ids = effort_models
        .iter()
        .map(|effort| effort.segment_id)
        .collect::<Vec<_>>();
    let overall_ranks_by_effort_id = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::SegmentId)
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?
        .into_iter()
        .fold(
            (
                HashMap::<i32, i32>::new(),
                None::<i32>,
                0,
            ),
            |(mut ranks, current_segment_id, current_rank), effort| {
                let next_rank = if current_segment_id == Some(effort.segment_id) {
                    current_rank + 1
                } else {
                    1
                };

                ranks.insert(effort.id, next_rank);

                (ranks, Some(effort.segment_id), next_rank)
            },
        )
        .0;
    let segment_titles_by_id = segments::Entity::find()
        .filter(segments::Column::Id.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|segment| (segment.id, segment.title))
        .collect::<HashMap<_, _>>();

    Ok(effort_models
        .into_iter()
        .filter_map(|effort| {
            let segment_title = segment_titles_by_id.get(&effort.segment_id)?.clone();

            Some(ActivitySegmentEffort {
                segment_id: effort.segment_id,
                segment_title,
                effort_index: effort.effort_index,
                duration_seconds: effort.duration_seconds,
                start_route_point_index: effort.start_route_point_index,
                end_route_point_index: effort.end_route_point_index,
                overall_rank: overall_ranks_by_effort_id.get(&effort.id).copied(),
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

    Ok(Json(ActivityResponse::from_detail(activity, segment_efforts)))
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

    delete_activity_with_derived_state(&state.db, &state.uploads_dir, user.id, activity).await?;

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
    let full_path = FsPath::new(&state.uploads_dir).join(&activity_import.storage_path);
    let bytes = tokio::fs::read(full_path).await?;
    let activity_draft = summarize_activity_upload(
        &activity_import.original_filename,
        &activity_import.format,
        &bytes,
    )?;
    let derived_data = derive_activity_detail_data(
        &activity_import.original_filename,
        &activity_import.format,
        &bytes,
    )?;
    let derived_data_json = serialize_derived_activity_data(&derived_data)?;

    let mut active_model: activities::ActiveModel = activity.into();
    active_model.title = Set(activity_draft.title);
    active_model.sport = Set(activity_draft.sport);
    active_model.original_filename = Set(Some(activity_import.original_filename));
    active_model.format = Set(Some(activity_import.format));
    active_model.started_at = Set(activity_draft.started_at);
    active_model.ended_at = Set(activity_draft.ended_at);
    active_model.distance_meters = Set(activity_draft.distance_meters);
    active_model.moving_time_seconds = Set(activity_draft.moving_time_seconds);
    active_model.total_time_seconds = Set(activity_draft.total_time_seconds);
    active_model.elevation_gain_meters = Set(activity_draft.elevation_gain_meters);
    active_model.elevation_loss_meters = Set(activity_draft.elevation_loss_meters);
    active_model.average_speed_mps = Set(activity_draft.average_speed_mps);
    active_model.max_speed_mps = Set(activity_draft.max_speed_mps);
    active_model.average_heart_rate_bpm = Set(activity_draft.average_heart_rate_bpm);
    active_model.max_heart_rate_bpm = Set(activity_draft.max_heart_rate_bpm);
    active_model.average_cadence_rpm = Set(activity_draft.average_cadence_rpm);
    active_model.max_cadence_rpm = Set(activity_draft.max_cadence_rpm);
    active_model.calories = Set(activity_draft.calories);
    active_model.derived_data_json = Set(Some(derived_data_json));

    let updated = active_model.update(&state.db).await?;

    refresh_activity_derived_state(
        &state.db,
        user.id,
        updated.id,
        &derived_data.route_points,
    )
    .await?;
    let segment_efforts = load_activity_segment_efforts(&state.db, user.id, updated.id).await?;

    Ok(Json(ActivityResponse::from_detail(updated, segment_efforts)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_details::serialize_derived_activity_data;
    use kaleido::glass::data::pagination::PaginatedResponse;

    #[test]
    fn activity_response_maps_model_fields() {
        let now = Utc::now();
        let response = ActivityResponse::from_detail(activities::Model {
            id: 42,
            user_id: 8,
            activity_import_id: Some(9),
            title: "Evening Ride".to_string(),
            sport: "ride".to_string(),
            source: "manual_upload".to_string(),
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
        }, vec![ActivitySegmentEffort {
            segment_id: 5,
            segment_title: "North Climb".to_string(),
            effort_index: 1,
            duration_seconds: 312,
            start_route_point_index: 0,
            end_route_point_index: 0,
            overall_rank: Some(1),
        }]);

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
        assert!(response.can_regenerate);
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

        assert_eq!(response.route_points.len(), MAX_ACTIVITY_STREAM_ROUTE_POINTS);
        assert_eq!(response.route_points.first().map(|point| point.elapsed_seconds), Some(0));
        assert_eq!(response.route_points.last().map(|point| point.elapsed_seconds), Some(39));
    }
}