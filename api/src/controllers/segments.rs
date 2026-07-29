use crate::activity_details::{
    derive_activity_detail_data, deserialize_derived_activity_data, ActivityRoutePoint,
};
use crate::activity_summary::summarize_activity_upload;
use crate::analytics::rebuild_activity_analytics_cache;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::dedupe::segment_dedupe_key;
use crate::entities::{
    activities, segment_efforts, segment_summaries, segment_user_summaries, segments,
};
use crate::segment_support::{
    deserialize_segment_route_points, serialize_segment_route_points, slice_effort_route_points,
};
use crate::storage::AppStorage;
use crate::tasks::QueuedTaskReference;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::entities::users;
use kaleido::auth::UserContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, IntoActiveModel,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

const SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS: f64 = 5.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SegmentMode {
    Xc,
    Dh,
}

impl SegmentMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Xc => "xc",
            Self::Dh => "dh",
        }
    }

    fn from_stored(value: &str) -> Self {
        match value {
            "dh" => Self::Dh,
            _ => Self::Xc,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentResponse {
    pub id: i32,
    pub title: String,
    pub source: String,
    pub mode: SegmentMode,
    pub starred: bool,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub distance_meters: Option<f64>,
    pub effort_count: i32,
    pub best_duration_seconds: Option<i32>,
    pub current_user_pr_duration_seconds: Option<i32>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_task_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub processing_task_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builder_source: Option<SegmentBuilderSourceResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<SegmentRoutePointResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<SegmentEffortResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentComparisonResponse {
    pub segment_id: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<SegmentRoutePointResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<SegmentEffortResponse>,
}

#[derive(Clone, Debug, FromQueryResult)]
struct SegmentListRow {
    id: i32,
    title: String,
    source: String,
    mode: String,
    starred: bool,
    original_filename: Option<String>,
    format: Option<String>,
    distance_meters: Option<f64>,
    source_activity_id: Option<i32>,
    source_start_route_point_index: Option<i32>,
    source_end_route_point_index: Option<i32>,
    last_activity_change_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, FromQueryResult)]
struct SegmentDedupeCandidateRow {
    id: i32,
    distance_meters: Option<f64>,
    route_data_json: Option<crate::activity_details::StoredRoutePointSeries>,
}

#[derive(Clone, Debug, FromQueryResult)]
struct EffortActivityRow {
    id: i32,
    title: String,
    started_at: DateTime<Utc>,
    derived_data_json: Option<crate::activity_details::StoredActivityDerivedData>,
}

#[derive(Clone, Debug, FromQueryResult)]
struct RiderNameRow {
    id: i32,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct SegmentBuilderSourceResponse {
    pub activity_id: i32,
    pub start_route_point_index: i32,
    pub end_route_point_index: i32,
}

fn segment_builder_source_from_model(
    segment: &segments::Model,
) -> Option<SegmentBuilderSourceResponse> {
    match (
        segment.source_activity_id,
        segment.source_start_route_point_index,
        segment.source_end_route_point_index,
    ) {
        (Some(activity_id), Some(start_route_point_index), Some(end_route_point_index)) => {
            Some(SegmentBuilderSourceResponse {
                activity_id,
                start_route_point_index,
                end_route_point_index,
            })
        }
        _ => None,
    }
}

fn segment_builder_source_from_values(
    activity_id: Option<i32>,
    start_route_point_index: Option<i32>,
    end_route_point_index: Option<i32>,
) -> Option<SegmentBuilderSourceResponse> {
    match (activity_id, start_route_point_index, end_route_point_index) {
        (Some(activity_id), Some(start_route_point_index), Some(end_route_point_index)) => {
            Some(SegmentBuilderSourceResponse {
                activity_id,
                start_route_point_index,
                end_route_point_index,
            })
        }
        _ => None,
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentEffortResponse {
    pub id: i32,
    pub rider_user_id: i32,
    pub activity_id: i32,
    pub activity_title: String,
    pub rider_name: String,
    pub activity_started_at: DateTime<Utc>,
    pub effort_index: i32,
    pub duration_seconds: i32,
    pub start_elapsed_seconds: i32,
    pub end_elapsed_seconds: i32,
    pub distance_meters: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<SegmentRoutePointResponse>,
}

#[derive(Debug, Clone, PartialEq, Serialize, ToSchema)]
pub struct SegmentRoutePointResponse {
    pub elapsed_seconds: i32,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance_meters: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation_meters: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed_mps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heart_rate_bpm: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSegmentFromActivityRequest {
    pub activity_id: i32,
    pub title: String,
    pub start_route_point_index: i32,
    pub end_route_point_index: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSegmentRequest {
    pub title: Option<String>,
    pub mode: Option<SegmentMode>,
    pub starred: Option<bool>,
}

#[utoipa::path(
    get,
    path = "/api/segments",
    responses(
        (status = 200, description = "Recent segments for the authenticated user", body = [SegmentResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_segments(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<SegmentResponse>>, AppError> {
    let mut segment_rows = segments::Entity::find()
        .select_only()
        .column(segments::Column::Id)
        .column(segments::Column::Title)
        .column(segments::Column::Source)
        .column(segments::Column::Mode)
        .column(segments::Column::Starred)
        .column(segments::Column::OriginalFilename)
        .column(segments::Column::Format)
        .column(segments::Column::DistanceMeters)
        .column(segments::Column::SourceActivityId)
        .column(segments::Column::SourceStartRoutePointIndex)
        .column(segments::Column::SourceEndRoutePointIndex)
        .column(segments::Column::LastActivityChangeAt)
        .column(segments::Column::CreatedAt)
        .filter(segments::Column::UserId.eq(user.id))
        .into_model::<SegmentListRow>()
        .all(&state.db)
        .await?;

    let segment_ids = segment_rows
        .iter()
        .map(|segment| segment.id)
        .collect::<Vec<_>>();
    let summary_by_segment_id = load_segment_summaries(&state.db, &segment_ids).await?;
    sort_segment_rows_by_latest_activity_started_at(&mut segment_rows, &summary_by_segment_id);
    let user_summary_by_segment_id =
        load_segment_user_summaries(&state.db, user.id, &segment_ids).await?;
    let stale_segment_ids = segment_rows
        .iter()
        .filter_map(|segment| {
            let summary = summary_by_segment_id.get(&segment.id);
            let user_summary = user_summary_by_segment_id.get(&segment.id);

            match (summary, user_summary) {
                (Some(summary), Some(user_summary))
                    if summary.updated_at >= segment.last_activity_change_at
                        && user_summary.updated_at >= segment.last_activity_change_at =>
                {
                    None
                }
                (Some(summary), None) if summary.updated_at >= segment.last_activity_change_at => {
                    None
                }
                _ => Some(segment.id),
            }
        })
        .collect::<Vec<_>>();

    if !stale_segment_ids.is_empty() {
        state
            .tasks
            .rebuild_segment_analytics(stale_segment_ids)
            .await;
    }

    Ok(Json(
        segment_rows
            .into_iter()
            .map(|segment| {
                let summary = summary_by_segment_id.get(&segment.id);
                let user_summary = user_summary_by_segment_id.get(&segment.id);
                let builder_source = segment_builder_source_from_values(
                    segment.source_activity_id,
                    segment.source_start_route_point_index,
                    segment.source_end_route_point_index,
                );

                SegmentResponse {
                    id: segment.id,
                    title: segment.title,
                    source: segment.source,
                    mode: SegmentMode::from_stored(&segment.mode),
                    starred: segment.starred,
                    original_filename: segment.original_filename,
                    format: segment.format,
                    distance_meters: segment.distance_meters,
                    effort_count: summary.map(|value| value.effort_count).unwrap_or_default(),
                    best_duration_seconds: summary.and_then(|value| value.best_duration_seconds),
                    current_user_pr_duration_seconds: user_summary
                        .and_then(|value| value.personal_best_duration_seconds),
                    created_at: segment.created_at,
                    processing_task_id: None,
                    processing_task_status: None,
                    builder_source,
                    route_points: Vec::new(),
                    efforts: Vec::new(),
                }
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/segments/{id}",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    responses(
        (status = 200, description = "Segment metadata and summary data", body = SegmentResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_segment(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<SegmentResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    Ok(Json(
        load_segment_summary_response(&state.db, &segment, user.id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/segments/{id}/comparison",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    responses(
        (status = 200, description = "Segment route and effort comparison samples", body = SegmentComparisonResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_segment_comparison(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<SegmentComparisonResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;

    Ok(Json(
        load_segment_comparison_response(&state.db, &segment).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/segments/{id}",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    request_body = UpdateSegmentRequest,
    responses(
        (status = 200, description = "Updated segment", body = SegmentResponse),
        (status = 400, description = "Invalid segment update", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_segment(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(payload): Json<UpdateSegmentRequest>,
) -> Result<Json<SegmentResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    let title = match payload.title.as_ref() {
        Some(value) => normalize_segment_title(value)?,
        None => segment.title.clone(),
    };
    let mode = payload
        .mode
        .unwrap_or_else(|| SegmentMode::from_stored(&segment.mode));
    let starred = payload.starred.unwrap_or(segment.starred);
    let title_changed = segment.title != title;
    let mode_changed = SegmentMode::from_stored(&segment.mode) != mode;
    let starred_changed = segment.starred != starred;

    if !title_changed && !mode_changed && !starred_changed {
        return Ok(Json(
            load_segment_summary_response(&state.db, &segment, user.id).await?,
        ));
    }

    let activity_ids = if title_changed {
        load_activity_ids_for_segments(&state.db, &[segment.id]).await?
    } else {
        Vec::new()
    };
    let txn = state.db.begin().await?;
    let mut active_segment = segment.into_active_model();
    active_segment.title = Set(title);
    active_segment.mode = Set(mode.as_str().to_string());
    active_segment.starred = Set(starred);
    let updated_segment = active_segment.update(&txn).await?;

    if title_changed && !activity_ids.is_empty() {
        rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
    }

    txn.commit().await?;

    Ok(Json(
        load_segment_summary_response(&state.db, &updated_segment, user.id).await?,
    ))
}

#[utoipa::path(
    put,
    path = "/api/segments/{id}/from-activity",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    request_body = CreateSegmentFromActivityRequest,
    responses(
        (status = 200, description = "Updated segment route from an activity slice", body = SegmentResponse),
        (status = 400, description = "Invalid activity slice", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment or activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn update_segment_from_activity(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(payload): Json<CreateSegmentFromActivityRequest>,
) -> Result<Json<SegmentResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(payload.activity_id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let title = normalize_segment_title(&payload.title)?;
    let activity_route_points =
        deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
    let segment_route_points = slice_builder_route_points(
        &activity_route_points,
        payload.start_route_point_index,
        payload.end_route_point_index,
    )?;
    let distance_meters = segment_route_points
        .last()
        .and_then(|point| point.distance_meters);

    if let Some(existing_segment) =
        find_duplicate_segment(&state.db, user.id, distance_meters, &segment_route_points).await?
    {
        if existing_segment.id != segment.id {
            return Err(AppError::validation_field(
                "activity_id",
                "Another segment already uses this route",
            ));
        }
    }

    let txn = state.db.begin().await?;
    let mut active_segment = segment.into_active_model();
    active_segment.title = Set(title);
    active_segment.source = Set("activity_segment_builder".to_string());
    active_segment.original_filename = Set(None);
    active_segment.format = Set(activity.format.clone());
    active_segment.distance_meters = Set(distance_meters);
    active_segment.route_data_json =
        Set(Some(serialize_segment_route_points(&segment_route_points)?));
    active_segment.source_activity_id = Set(Some(activity.id));
    active_segment.source_start_route_point_index = Set(Some(payload.start_route_point_index));
    active_segment.source_end_route_point_index = Set(Some(payload.end_route_point_index));
    let updated_segment = active_segment.update(&txn).await?;
    txn.commit().await?;
    let processing_task = enqueue_segment_effort_regeneration(&state, updated_segment.id).await?;

    Ok(Json(load_light_segment_response(
        &updated_segment,
        Some(&processing_task),
    )))
}

#[utoipa::path(
    delete,
    path = "/api/segments/{id}",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    responses(
        (status = 204, description = "Segment deleted"),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn delete_segment(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<StatusCode, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;

    delete_segment_with_related_state(&state.db, segment.id).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/segments/from-activity",
    request_body = CreateSegmentFromActivityRequest,
    responses(
        (status = 201, description = "Segment created from an activity route slice", body = SegmentResponse),
        (status = 200, description = "Matching segment already exists", body = SegmentResponse),
        (status = 400, description = "Invalid activity slice", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_segment_from_activity(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(payload): Json<CreateSegmentFromActivityRequest>,
) -> Result<(StatusCode, Json<SegmentResponse>), AppError> {
    let activity = activities::Entity::find()
        .filter(activities::Column::Id.eq(payload.activity_id))
        .filter(activities::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let title = normalize_segment_title(&payload.title)?;
    let activity_route_points =
        deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
    let segment_route_points = slice_builder_route_points(
        &activity_route_points,
        payload.start_route_point_index,
        payload.end_route_point_index,
    )?;
    let distance_meters = segment_route_points
        .last()
        .and_then(|point| point.distance_meters);

    if let Some(existing_segment) =
        find_duplicate_segment(&state.db, user.id, distance_meters, &segment_route_points).await?
    {
        let should_update_title = existing_segment.title != title;
        let should_update_builder_source = existing_segment.source_activity_id != Some(activity.id)
            || existing_segment.source_start_route_point_index
                != Some(payload.start_route_point_index)
            || existing_segment.source_end_route_point_index != Some(payload.end_route_point_index);
        let existing_segment = if should_update_title || should_update_builder_source {
            let activity_ids = if should_update_title {
                load_activity_ids_for_segments(&state.db, &[existing_segment.id]).await?
            } else {
                Vec::new()
            };
            let txn = state.db.begin().await?;
            let mut active_segment = existing_segment.into_active_model();

            if should_update_title {
                active_segment.title = Set(title.clone());
            }

            active_segment.source_activity_id = Set(Some(activity.id));
            active_segment.source_start_route_point_index =
                Set(Some(payload.start_route_point_index));
            active_segment.source_end_route_point_index = Set(Some(payload.end_route_point_index));

            let updated_segment = active_segment.update(&txn).await?;

            if !activity_ids.is_empty() {
                rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
            }

            txn.commit().await?;

            updated_segment
        } else {
            existing_segment
        };

        return Ok((
            StatusCode::OK,
            Json(load_segment_summary_response(&state.db, &existing_segment, user.id).await?),
        ));
    }

    let segment = segments::ActiveModel {
        user_id: Set(user.id),
        title: Set(title),
        source: Set("activity_segment_builder".to_string()),
        mode: Set(SegmentMode::Xc.as_str().to_string()),
        starred: Set(false),
        original_filename: Set(None),
        format: Set(activity.format.clone()),
        distance_meters: Set(distance_meters),
        route_data_json: Set(Some(serialize_segment_route_points(&segment_route_points)?)),
        source_activity_id: Set(Some(activity.id)),
        source_start_route_point_index: Set(Some(payload.start_route_point_index)),
        source_end_route_point_index: Set(Some(payload.end_route_point_index)),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    let processing_task = enqueue_segment_effort_regeneration(&state, segment.id).await?;

    Ok((
        StatusCode::CREATED,
        Json(load_light_segment_response(
            &segment,
            Some(&processing_task),
        )),
    ))
}

#[utoipa::path(
    post,
    path = "/api/segments",
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Segment imported and matched to recent activities", body = SegmentResponse),
        (status = 400, description = "Invalid upload", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn import_segment(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<SegmentResponse>), AppError> {
    let upload = read_uploaded_segment_file(multipart).await?;
    let segment_summary =
        summarize_activity_upload(&upload.original_filename, &upload.format, &upload.bytes)?;
    let segment_detail =
        derive_activity_detail_data(&upload.original_filename, &upload.format, &upload.bytes)?;

    if segment_detail.route_points.len() < 2 {
        return Err(AppError::validation_field(
            "file",
            "Segment imports require GPX or TCX route data with coordinates",
        ));
    }

    if let Some(existing_segment) = find_duplicate_segment(
        &state.db,
        user.id,
        segment_summary.distance_meters,
        &segment_detail.route_points,
    )
    .await?
    {
        return Ok((
            StatusCode::OK,
            Json(load_segment_summary_response(&state.db, &existing_segment, user.id).await?),
        ));
    }

    let segment = segments::ActiveModel {
        user_id: Set(user.id),
        title: Set(segment_summary.title),
        source: Set("manual_segment_import".to_string()),
        mode: Set(SegmentMode::Xc.as_str().to_string()),
        starred: Set(false),
        original_filename: Set(Some(upload.original_filename)),
        format: Set(Some(upload.format)),
        distance_meters: Set(segment_summary.distance_meters),
        route_data_json: Set(Some(serialize_segment_route_points(
            &segment_detail.route_points,
        )?)),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    let processing_task = enqueue_segment_effort_regeneration(&state, segment.id).await?;

    Ok((
        StatusCode::CREATED,
        Json(load_light_segment_response(
            &segment,
            Some(&processing_task),
        )),
    ))
}

struct UploadedSegmentFile {
    original_filename: String,
    format: String,
    bytes: Vec<u8>,
}

async fn read_uploaded_segment_file(
    mut multipart: Multipart,
) -> Result<UploadedSegmentFile, AppError> {
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::bad_request(format!("Malformed multipart payload: {error}")))?
    {
        if field.name() != Some("file") && field.file_name().is_none() {
            continue;
        }

        let original_filename = field
            .file_name()
            .map(|value| value.to_string())
            .ok_or_else(|| {
                AppError::validation_field("file", "Uploaded file is missing a filename")
            })?;
        let format = validate_segment_format(&original_filename)?;
        let mut bytes = Vec::new();

        while let Some(chunk) = field.chunk().await.map_err(|error| {
            AppError::bad_request(format!("Failed to read segment upload: {error}"))
        })? {
            bytes.extend_from_slice(&chunk);
        }

        if bytes.is_empty() {
            return Err(AppError::validation_field("file", "Uploaded file is empty"));
        }

        return Ok(UploadedSegmentFile {
            original_filename,
            format,
            bytes,
        });
    }

    Err(AppError::validation_field(
        "file",
        "A GPX or TCX file is required to import a segment",
    ))
}

fn validate_segment_format(filename: &str) -> Result<String, AppError> {
    let extension = std::path::Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            AppError::validation_field(
                "file",
                "Segments currently require a GPX or TCX export with route coordinates",
            )
        })?;

    match extension.as_str() {
        "gpx" | "tcx" => Ok(extension),
        _ => Err(AppError::validation_field(
            "file",
            "Segments currently require a GPX or TCX export with route coordinates",
        )),
    }
}

fn normalize_segment_title(raw_title: &str) -> Result<String, AppError> {
    let title = raw_title.trim();

    if title.is_empty() {
        return Err(AppError::validation_field(
            "title",
            "Segment name is required",
        ));
    }

    Ok(title.to_string())
}

fn slice_builder_route_points(
    route_points: &[ActivityRoutePoint],
    start_route_point_index: i32,
    end_route_point_index: i32,
) -> Result<Vec<ActivityRoutePoint>, AppError> {
    if route_points.len() < 2 {
        return Err(AppError::validation_field(
            "activity_id",
            "Selected activity does not have enough route data to build a segment",
        ));
    }

    let start_index = usize::try_from(start_route_point_index).map_err(|_| {
        AppError::validation_field(
            "start_route_point_index",
            "Segment start must be within the selected activity route",
        )
    })?;
    let end_index = usize::try_from(end_route_point_index).map_err(|_| {
        AppError::validation_field(
            "end_route_point_index",
            "Segment end must be within the selected activity route",
        )
    })?;

    if start_index >= route_points.len() {
        return Err(AppError::validation_field(
            "start_route_point_index",
            "Segment start must be within the selected activity route",
        ));
    }

    if end_index >= route_points.len() {
        return Err(AppError::validation_field(
            "end_route_point_index",
            "Segment end must be within the selected activity route",
        ));
    }

    if start_index >= end_index {
        return Err(AppError::validation_field(
            "start_route_point_index",
            "Segment start must come before the end",
        ));
    }

    Ok(slice_effort_route_points(
        route_points,
        start_route_point_index,
        end_route_point_index,
    ))
}

async fn find_duplicate_segment(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    distance_meters: Option<f64>,
    route_points: &[ActivityRoutePoint],
) -> Result<Option<segments::Model>, AppError> {
    let Some(target_key) = segment_dedupe_key(distance_meters, route_points) else {
        return Ok(None);
    };

    let mut candidate_query = segments::Entity::find()
        .select_only()
        .column(segments::Column::Id)
        .column(segments::Column::DistanceMeters)
        .column(segments::Column::RouteDataJson)
        .filter(segments::Column::UserId.eq(user_id));

    if let Some((minimum_distance, maximum_distance)) =
        segment_dedupe_distance_bucket(distance_meters)
    {
        candidate_query = candidate_query
            .filter(segments::Column::DistanceMeters.gte(minimum_distance))
            .filter(segments::Column::DistanceMeters.lt(maximum_distance));
    } else {
        candidate_query = candidate_query.filter(segments::Column::DistanceMeters.is_null());
    }

    let candidates = candidate_query
        .into_model::<SegmentDedupeCandidateRow>()
        .all(db)
        .await?;

    for segment in candidates {
        let candidate_route_points =
            deserialize_segment_route_points(segment.route_data_json.as_ref());
        if segment_dedupe_key(segment.distance_meters, &candidate_route_points).as_deref()
            == Some(target_key.as_str())
        {
            return segments::Entity::find()
                .filter(segments::Column::Id.eq(segment.id))
                .filter(segments::Column::UserId.eq(user_id))
                .one(db)
                .await
                .map_err(AppError::from);
        }
    }

    Ok(None)
}

fn segment_dedupe_distance_bucket(distance_meters: Option<f64>) -> Option<(f64, f64)> {
    let distance_meters = distance_meters.filter(|value| value.is_finite() && *value > 0.0)?;
    let minimum = (distance_meters / SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS).floor()
        * SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS;

    Some((minimum, minimum + SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS))
}

async fn load_activity_ids_for_segments<C>(
    db: &C,
    segment_ids: &[i32],
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait,
{
    if segment_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut activity_ids = segment_efforts::Entity::find()
        .select_only()
        .column(segment_efforts::Column::ActivityId)
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .into_tuple::<i32>()
        .all(db)
        .await?;

    activity_ids.sort_unstable();
    activity_ids.dedup();

    Ok(activity_ids)
}

async fn delete_segment_with_related_state(
    db: &sea_orm::DatabaseConnection,
    segment_id: i32,
) -> Result<(), AppError> {
    let activity_ids = load_activity_ids_for_segments(db, &[segment_id]).await?;
    let txn = db.begin().await?;

    segment_user_summaries::Entity::delete_many()
        .filter(segment_user_summaries::Column::SegmentId.eq(segment_id))
        .exec(&txn)
        .await?;

    segment_summaries::Entity::delete_many()
        .filter(segment_summaries::Column::SegmentId.eq(segment_id))
        .exec(&txn)
        .await?;

    segment_efforts::Entity::delete_many()
        .filter(segment_efforts::Column::SegmentId.eq(segment_id))
        .exec(&txn)
        .await?;

    segments::Entity::delete_many()
        .filter(segments::Column::Id.eq(segment_id))
        .exec(&txn)
        .await?;

    if !activity_ids.is_empty() {
        rebuild_activity_analytics_cache(&txn, &activity_ids).await?;
    }

    txn.commit().await?;

    Ok(())
}

async fn load_segment_summary_response(
    db: &sea_orm::DatabaseConnection,
    segment: &segments::Model,
    user_id: i32,
) -> Result<SegmentResponse, AppError> {
    let summary_by_segment_id = load_segment_summaries(db, &[segment.id]).await?;
    let user_summary_by_segment_id =
        load_segment_user_summaries(db, user_id, &[segment.id]).await?;
    let summary = summary_by_segment_id.get(&segment.id);
    let user_summary = user_summary_by_segment_id.get(&segment.id);

    Ok(SegmentResponse {
        id: segment.id,
        title: segment.title.clone(),
        source: segment.source.clone(),
        mode: SegmentMode::from_stored(&segment.mode),
        starred: segment.starred,
        original_filename: segment.original_filename.clone(),
        format: segment.format.clone(),
        distance_meters: segment.distance_meters,
        effort_count: summary.map(|value| value.effort_count).unwrap_or_default(),
        best_duration_seconds: summary.and_then(|value| value.best_duration_seconds),
        current_user_pr_duration_seconds: user_summary
            .and_then(|value| value.personal_best_duration_seconds),
        created_at: segment.created_at,
        processing_task_id: None,
        processing_task_status: None,
        builder_source: segment_builder_source_from_model(segment),
        route_points: Vec::new(),
        efforts: Vec::new(),
    })
}

async fn load_segment_comparison_response(
    db: &sea_orm::DatabaseConnection,
    segment: &segments::Model,
) -> Result<SegmentComparisonResponse, AppError> {
    let route_points = segment_route_point_responses(&deserialize_segment_route_points(
        segment.route_data_json.as_ref(),
    ));
    let efforts = load_effort_responses(db, &[segment.id]).await?;

    Ok(SegmentComparisonResponse {
        segment_id: segment.id,
        route_points,
        efforts,
    })
}

fn segment_route_point_responses(
    route_points: &[ActivityRoutePoint],
) -> Vec<SegmentRoutePointResponse> {
    route_points
        .iter()
        .map(SegmentRoutePointResponse::from_activity_route_point)
        .collect()
}

impl SegmentRoutePointResponse {
    fn from_activity_route_point(point: &ActivityRoutePoint) -> Self {
        Self {
            elapsed_seconds: point.elapsed_seconds,
            latitude: point.latitude,
            longitude: point.longitude,
            distance_meters: point.distance_meters,
            elevation_meters: point.elevation_meters,
            speed_mps: point.speed_mps,
            heart_rate_bpm: point.heart_rate_bpm,
        }
    }
}

fn load_light_segment_response(
    segment: &segments::Model,
    processing_task: Option<&QueuedTaskReference>,
) -> SegmentResponse {
    SegmentResponse {
        id: segment.id,
        title: segment.title.clone(),
        source: segment.source.clone(),
        mode: SegmentMode::from_stored(&segment.mode),
        starred: segment.starred,
        original_filename: segment.original_filename.clone(),
        format: segment.format.clone(),
        distance_meters: segment.distance_meters,
        effort_count: 0,
        best_duration_seconds: None,
        current_user_pr_duration_seconds: None,
        created_at: segment.created_at,
        processing_task_id: processing_task.map(|task| task.id.clone()),
        processing_task_status: processing_task.map(|task| task.status.clone()),
        builder_source: segment_builder_source_from_model(segment),
        route_points: Vec::new(),
        efforts: Vec::new(),
    }
}

async fn enqueue_segment_effort_regeneration(
    state: &AppStorage,
    segment_id: i32,
) -> Result<QueuedTaskReference, AppError> {
    state
        .tasks
        .regenerate_segment_efforts(segment_id)
        .await
        .map_err(|message| {
            AppError::internal(format!(
                "Failed to queue segment effort regeneration: {message}"
            ))
        })
}

#[cfg(test)]
fn sort_segments_by_latest_activity_started_at(
    segment_models: &mut [segments::Model],
    summary_by_segment_id: &HashMap<i32, segment_summaries::Model>,
) {
    segment_models.sort_by(|left, right| {
        let left_latest = summary_by_segment_id
            .get(&left.id)
            .and_then(|summary| summary.latest_activity_started_at);
        let right_latest = summary_by_segment_id
            .get(&right.id)
            .and_then(|summary| summary.latest_activity_started_at);

        right_latest
            .cmp(&left_latest)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.id.cmp(&left.id))
    });
}

fn sort_segment_rows_by_latest_activity_started_at(
    segment_rows: &mut [SegmentListRow],
    summary_by_segment_id: &HashMap<i32, segment_summaries::Model>,
) {
    segment_rows.sort_by(|left, right| {
        let left_latest = summary_by_segment_id
            .get(&left.id)
            .and_then(|summary| summary.latest_activity_started_at);
        let right_latest = summary_by_segment_id
            .get(&right.id)
            .and_then(|summary| summary.latest_activity_started_at);

        right_latest
            .cmp(&left_latest)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| right.id.cmp(&left.id))
    });
}

async fn load_segment_summaries(
    db: &sea_orm::DatabaseConnection,
    segment_ids: &[i32],
) -> Result<HashMap<i32, segment_summaries::Model>, AppError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(segment_summaries::Entity::find()
        .filter(segment_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|summary| (summary.segment_id, summary))
        .collect())
}

async fn load_segment_user_summaries(
    db: &sea_orm::DatabaseConnection,
    user_id: i32,
    segment_ids: &[i32],
) -> Result<HashMap<i32, segment_user_summaries::Model>, AppError> {
    if segment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    Ok(segment_user_summaries::Entity::find()
        .filter(segment_user_summaries::Column::UserId.eq(user_id))
        .filter(segment_user_summaries::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .all(db)
        .await?
        .into_iter()
        .map(|summary| (summary.segment_id, summary))
        .collect())
}

async fn load_effort_responses(
    db: &sea_orm::DatabaseConnection,
    segment_ids: &[i32],
) -> Result<Vec<SegmentEffortResponse>, AppError> {
    if segment_ids.is_empty() {
        return Ok(Vec::new());
    }

    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.is_in(segment_ids.iter().copied()))
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;
    let activity_ids = efforts
        .iter()
        .map(|effort| effort.activity_id)
        .collect::<Vec<_>>();
    let rider_user_ids = efforts
        .iter()
        .map(|effort| effort.user_id)
        .collect::<Vec<_>>();
    let activity_models = activities::Entity::find()
        .select_only()
        .column(activities::Column::Id)
        .column(activities::Column::Title)
        .column(activities::Column::StartedAt)
        .column(activities::Column::DerivedDataJson)
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .into_model::<EffortActivityRow>()
        .all(db)
        .await?;
    let rider_models = users::Entity::find()
        .select_only()
        .column(users::Column::Id)
        .column(users::Column::Name)
        .filter(users::Column::Id.is_in(rider_user_ids.iter().copied()))
        .into_model::<RiderNameRow>()
        .all(db)
        .await?;
    let activities_by_id = activity_models
        .into_iter()
        .map(|activity| (activity.id, activity))
        .collect::<HashMap<_, _>>();
    let riders_by_id = rider_models
        .into_iter()
        .map(|rider| (rider.id, rider))
        .collect::<HashMap<_, _>>();

    Ok(efforts
        .into_iter()
        .filter_map(|effort| {
            let activity = activities_by_id.get(&effort.activity_id)?;
            let rider = riders_by_id.get(&effort.user_id)?;
            let derived_data =
                deserialize_derived_activity_data(activity.derived_data_json.as_ref());

            let route_points = slice_effort_route_points(
                &derived_data.route_points,
                effort.start_route_point_index,
                effort.end_route_point_index,
            );

            Some(SegmentEffortResponse {
                id: effort.id,
                rider_user_id: effort.user_id,
                activity_id: effort.activity_id,
                activity_title: activity.title.clone(),
                rider_name: rider.name.clone(),
                activity_started_at: activity.started_at,
                effort_index: effort.effort_index,
                duration_seconds: effort.duration_seconds,
                start_elapsed_seconds: effort.start_elapsed_seconds,
                end_elapsed_seconds: effort.end_elapsed_seconds,
                distance_meters: effort.distance_meters,
                route_points: segment_route_point_responses(&route_points),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn build_route_point(
        elapsed_seconds: i32,
        distance_meters: Option<f64>,
        latitude: f64,
        longitude: f64,
    ) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds,
            latitude,
            longitude,
            distance_meters,
            elevation_meters: None,
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_watts: None,
        }
    }

    fn current_user_pr_duration_from_models(
        efforts: &[segment_efforts::Model],
        user_id: i32,
    ) -> Option<i32> {
        efforts
            .iter()
            .filter(|effort| effort.user_id == user_id)
            .map(|effort| effort.duration_seconds)
            .min()
    }

    fn build_segment_model(id: i32, created_at: DateTime<Utc>) -> segments::Model {
        segments::Model {
            id,
            user_id: 1,
            title: format!("Segment {id}"),
            source: "manual_segment_import".to_string(),
            mode: SegmentMode::Xc.as_str().to_string(),
            starred: false,
            original_filename: None,
            format: Some("gpx".to_string()),
            distance_meters: Some(1800.0),
            route_data_json: None,
            source_activity_id: None,
            source_start_route_point_index: None,
            source_end_route_point_index: None,
            last_activity_change_at: created_at,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn validate_segment_format_accepts_route_files() {
        assert_eq!(validate_segment_format("climb.gpx").unwrap(), "gpx");
        assert_eq!(validate_segment_format("climb.tcx").unwrap(), "tcx");
    }

    #[test]
    fn validate_segment_format_rejects_fit_for_now() {
        let error = validate_segment_format("climb.fit").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "Segments currently require a GPX or TCX export with route coordinates"
        );
    }

    #[test]
    fn normalize_segment_title_trims_whitespace() {
        assert_eq!(
            normalize_segment_title("  Main Street Rise  ").unwrap(),
            "Main Street Rise"
        );
    }

    #[test]
    fn normalize_segment_title_rejects_blank_titles() {
        let error = normalize_segment_title("   ").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Segment name is required");
    }

    #[test]
    fn segment_mode_from_stored_defaults_to_xc() {
        assert_eq!(SegmentMode::from_stored("xc"), SegmentMode::Xc);
        assert_eq!(SegmentMode::from_stored("dh"), SegmentMode::Dh);
        assert_eq!(SegmentMode::from_stored("unknown"), SegmentMode::Xc);
    }

    #[test]
    fn segment_route_point_responses_preserve_all_points() {
        let route_points = (0..25)
            .map(|index| {
                build_route_point(
                    index,
                    Some(index as f64 * 10.0),
                    45.0 + index as f64 * 0.001,
                    -122.0 - index as f64 * 0.001,
                )
            })
            .collect::<Vec<_>>();

        let response_points = segment_route_point_responses(&route_points);

        assert_eq!(response_points.len(), route_points.len());
        assert_eq!(
            response_points.first().map(|point| point.elapsed_seconds),
            Some(0)
        );
        assert_eq!(
            response_points.last().map(|point| point.elapsed_seconds),
            Some(24)
        );
    }

    #[test]
    fn segment_route_point_response_preserves_precision_and_omits_unused_telemetry() {
        let point = ActivityRoutePoint {
            elapsed_seconds: 12,
            latitude: 45.12345678,
            longitude: -122.87654321,
            distance_meters: Some(123.4567),
            elevation_meters: Some(987.6543),
            speed_mps: Some(4.567),
            heart_rate_bpm: Some(151),
            cadence_rpm: Some(88),
            power_watts: Some(240),
        };

        let response = SegmentRoutePointResponse::from_activity_route_point(&point);
        let serialized = serde_json::to_value(response).unwrap();

        assert_eq!(serialized["latitude"], serde_json::json!(45.12345678));
        assert_eq!(serialized["longitude"], serde_json::json!(-122.87654321));
        assert_eq!(serialized["distance_meters"], serde_json::json!(123.4567));
        assert_eq!(serialized["elevation_meters"], serde_json::json!(987.6543));
        assert_eq!(serialized["speed_mps"], serde_json::json!(4.567));
        assert_eq!(serialized["heart_rate_bpm"], serde_json::json!(151));
        assert!(serialized.get("cadence_rpm").is_none());
        assert!(serialized.get("power_watts").is_none());
    }

    #[test]
    fn load_activity_ids_for_segments_dedupes_activity_ids() {
        let mut activity_ids = vec![44, 12, 44, 19, 12];

        activity_ids.sort_unstable();
        activity_ids.dedup();

        assert_eq!(activity_ids, vec![12, 19, 44]);
    }

    #[test]
    fn segment_builder_source_from_model_requires_complete_metadata() {
        let created_at = Utc::now();
        let mut partial = build_segment_model(1, created_at);
        partial.source_activity_id = Some(99);
        partial.source_start_route_point_index = Some(12);

        assert_eq!(segment_builder_source_from_model(&partial), None);

        let mut complete = build_segment_model(2, created_at);
        complete.source_activity_id = Some(42);
        complete.source_start_route_point_index = Some(8);
        complete.source_end_route_point_index = Some(24);

        assert_eq!(
            segment_builder_source_from_model(&complete),
            Some(SegmentBuilderSourceResponse {
                activity_id: 42,
                start_route_point_index: 8,
                end_route_point_index: 24,
            })
        );
    }

    #[test]
    fn slice_builder_route_points_normalizes_selected_route_window() {
        let route_points = vec![
            build_route_point(0, Some(0.0), 44.0, -93.0),
            build_route_point(12, Some(150.0), 44.001, -93.001),
            build_route_point(28, Some(410.0), 44.002, -93.002),
        ];
        let sliced = slice_builder_route_points(&route_points, 1, 2).unwrap();

        assert_eq!(sliced.len(), 2);
        assert_eq!(sliced[0].elapsed_seconds, 0);
        assert_eq!(sliced[0].distance_meters, Some(0.0));
        assert_eq!(sliced[1].elapsed_seconds, 16);
        assert_eq!(sliced[1].distance_meters, Some(260.0));
    }

    #[test]
    fn slice_builder_route_points_rejects_reversed_indexes() {
        let route_points = vec![
            build_route_point(0, Some(0.0), 44.0, -93.0),
            build_route_point(12, Some(150.0), 44.001, -93.001),
            build_route_point(28, Some(410.0), 44.002, -93.002),
        ];
        let error = slice_builder_route_points(&route_points, 2, 1).unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Segment start must come before the end");
    }

    #[test]
    fn loads_current_user_pr_duration_from_models() {
        let now = Utc::now();
        let efforts = vec![
            segment_efforts::Model {
                id: 1,
                segment_id: 10,
                user_id: 7,
                activity_id: 100,
                effort_index: 1,
                duration_seconds: 320,
                start_elapsed_seconds: 0,
                end_elapsed_seconds: 320,
                start_route_point_index: 0,
                end_route_point_index: 1,
                distance_meters: Some(1800.0),
                overall_rank: None,
                user_rank: None,
                created_at: now,
                updated_at: now,
            },
            segment_efforts::Model {
                id: 2,
                segment_id: 10,
                user_id: 7,
                activity_id: 101,
                effort_index: 2,
                duration_seconds: 305,
                start_elapsed_seconds: 0,
                end_elapsed_seconds: 305,
                start_route_point_index: 0,
                end_route_point_index: 1,
                distance_meters: Some(1800.0),
                overall_rank: None,
                user_rank: None,
                created_at: now,
                updated_at: now,
            },
            segment_efforts::Model {
                id: 3,
                segment_id: 10,
                user_id: 9,
                activity_id: 102,
                effort_index: 1,
                duration_seconds: 300,
                start_elapsed_seconds: 0,
                end_elapsed_seconds: 300,
                start_route_point_index: 0,
                end_route_point_index: 1,
                distance_meters: Some(1800.0),
                overall_rank: None,
                user_rank: None,
                created_at: now,
                updated_at: now,
            },
        ];

        assert_eq!(current_user_pr_duration_from_models(&efforts, 7), Some(305));
        assert_eq!(current_user_pr_duration_from_models(&efforts, 11), None);
    }

    #[test]
    fn sorts_segments_by_latest_activity_started_at_then_created_at() {
        let base_time = Utc::now();
        let mut segment_models = vec![
            build_segment_model(1, base_time + Duration::minutes(1)),
            build_segment_model(2, base_time + Duration::minutes(2)),
            build_segment_model(3, base_time + Duration::minutes(3)),
            build_segment_model(4, base_time + Duration::minutes(4)),
        ];
        let summary_by_segment_id = HashMap::from([
            (
                1,
                segment_summaries::Model {
                    segment_id: 1,
                    effort_count: 3,
                    leader_user_id: Some(9),
                    leader_effort_id: Some(11),
                    best_duration_seconds: Some(300),
                    latest_activity_started_at: Some(base_time + Duration::days(1)),
                    latest_activity_id: Some(101),
                    latest_effort_id: Some(201),
                    created_at: base_time,
                    updated_at: base_time,
                },
            ),
            (
                2,
                segment_summaries::Model {
                    segment_id: 2,
                    effort_count: 5,
                    leader_user_id: Some(9),
                    leader_effort_id: Some(12),
                    best_duration_seconds: Some(290),
                    latest_activity_started_at: Some(base_time + Duration::days(2)),
                    latest_activity_id: Some(102),
                    latest_effort_id: Some(202),
                    created_at: base_time,
                    updated_at: base_time,
                },
            ),
            (
                3,
                segment_summaries::Model {
                    segment_id: 3,
                    effort_count: 4,
                    leader_user_id: Some(8),
                    leader_effort_id: Some(13),
                    best_duration_seconds: Some(295),
                    latest_activity_started_at: Some(base_time + Duration::days(2)),
                    latest_activity_id: Some(103),
                    latest_effort_id: Some(203),
                    created_at: base_time,
                    updated_at: base_time,
                },
            ),
        ]);

        sort_segments_by_latest_activity_started_at(&mut segment_models, &summary_by_segment_id);

        assert_eq!(
            segment_models
                .into_iter()
                .map(|segment| segment.id)
                .collect::<Vec<_>>(),
            vec![3, 2, 1, 4]
        );
    }
}
