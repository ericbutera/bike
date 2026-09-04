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
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Datelike, Utc};
use kaleido::auth::entities::users;
use kaleido::auth::UserContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, IntoActiveModel,
    QueryFilter, QueryOrder, QuerySelect, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

const SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS: f64 = 5.0;
const DEFAULT_ANALYSIS_SPLIT_COUNT: usize = 10;
const MIN_ANALYSIS_SPLIT_COUNT: usize = 2;
const MAX_ANALYSIS_SPLIT_COUNT: usize = 30;
const TOP_ANALYSIS_SECTION_EFFORT_COUNT: usize = 5;

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

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentYearlyBestsResponse {
    pub segment_id: i32,
    pub segment_title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub years: Vec<SegmentYearlyBestResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct SegmentYearlyBestResponse {
    pub year: i32,
    pub effort_id: i32,
    pub activity_id: i32,
    pub activity_title: String,
    pub activity_started_at: DateTime<Utc>,
    pub effort_index: i32,
    pub duration_seconds: i32,
    pub improvement_from_previous_year_seconds: Option<i32>,
    pub improvement_from_first_year_seconds: Option<i32>,
}

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
pub struct SegmentEffortAnalysisQuery {
    pub reference_effort_id: Option<i32>,
    pub split_count: Option<usize>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SegmentEffortAnalysisResponse {
    pub segment_id: i32,
    pub segment_title: String,
    pub split_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<SegmentRoutePointResponse>,
    pub reference_effort: SegmentAnalysisEffortSummaryResponse,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<SegmentAnalysisEffortSummaryResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<SegmentAnalysisSectionResponse>,
    pub theoretical_best_duration_seconds: f64,
    pub theoretical_best_gain_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct SegmentAnalysisEffortSummaryResponse {
    pub effort_id: i32,
    pub activity_id: i32,
    pub activity_title: String,
    pub activity_started_at: DateTime<Utc>,
    pub effort_index: i32,
    pub duration_seconds: i32,
    pub delta_from_reference_seconds: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct SegmentAnalysisSectionResponse {
    pub section_index: usize,
    pub start_progress_percent: f64,
    pub end_progress_percent: f64,
    pub reference_split_seconds: f64,
    pub best_split_seconds: f64,
    pub best_effort_id: i32,
    pub best_activity_id: i32,
    pub best_activity_title: String,
    pub gain_available_seconds: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub top_efforts: Vec<SegmentAnalysisSectionEffortResponse>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub efforts: Vec<SegmentAnalysisSectionEffortResponse>,
}

#[derive(Clone, Debug, PartialEq, Serialize, ToSchema)]
pub struct SegmentAnalysisSectionEffortResponse {
    pub effort_id: i32,
    pub activity_id: i32,
    pub activity_title: String,
    pub activity_started_at: DateTime<Utc>,
    pub split_seconds: f64,
    pub delta_from_reference_seconds: f64,
    pub delta_from_best_seconds: f64,
    pub average_speed_mps: Option<f64>,
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
struct EffortActivitySummaryRow {
    id: i32,
    title: String,
    started_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct SegmentAnalysisEffortSource {
    effort_id: i32,
    activity_id: i32,
    activity_title: String,
    activity_started_at: DateTime<Utc>,
    effort_index: i32,
    duration_seconds: i32,
    route_points: Vec<ActivityRoutePoint>,
}

#[derive(Clone, Debug)]
struct SegmentAnalysisSample {
    elapsed_seconds: f64,
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
    get,
    path = "/api/segments/{id}/yearly-bests",
    params(
        ("id" = i32, Path, description = "Segment ID")
    ),
    responses(
        (status = 200, description = "Fastest authenticated rider effort per year for one segment", body = SegmentYearlyBestsResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_segment_yearly_bests(
    Path(id): Path<i32>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<SegmentYearlyBestsResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;

    Ok(Json(
        load_segment_yearly_bests_response(&state.db, &segment, user.id).await?,
    ))
}

#[utoipa::path(
    get,
    path = "/api/segments/{id}/effort-analysis",
    params(
        ("id" = i32, Path, description = "Segment ID"),
        SegmentEffortAnalysisQuery
    ),
    responses(
        (status = 200, description = "Distance-normalized effort split analysis for one segment", body = SegmentEffortAnalysisResponse),
        (status = 400, description = "Invalid analysis options", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Segment or reference effort not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "segments",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_segment_effort_analysis(
    Path(id): Path<i32>,
    Query(query): Query<SegmentEffortAnalysisQuery>,
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<SegmentEffortAnalysisResponse>, AppError> {
    let segment = segments::Entity::find()
        .filter(segments::Column::Id.eq(id))
        .filter(segments::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Segment not found"))?;

    Ok(Json(
        load_segment_effort_analysis_response(&state.db, &segment, user.id, query).await?,
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

async fn load_segment_yearly_bests_response(
    db: &sea_orm::DatabaseConnection,
    segment: &segments::Model,
    user_id: i32,
) -> Result<SegmentYearlyBestsResponse, AppError> {
    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.eq(segment.id))
        .filter(segment_efforts::Column::UserId.eq(user_id))
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;
    if efforts.is_empty() {
        return Ok(SegmentYearlyBestsResponse {
            segment_id: segment.id,
            segment_title: segment.title.clone(),
            years: Vec::new(),
        });
    }

    let activity_ids = efforts
        .iter()
        .map(|effort| effort.activity_id)
        .collect::<Vec<_>>();
    let activity_models = activities::Entity::find()
        .select_only()
        .column(activities::Column::Id)
        .column(activities::Column::Title)
        .column(activities::Column::StartedAt)
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .into_model::<EffortActivitySummaryRow>()
        .all(db)
        .await?;
    let activities_by_id = activity_models
        .into_iter()
        .map(|activity| (activity.id, activity))
        .collect::<HashMap<_, _>>();

    Ok(SegmentYearlyBestsResponse {
        segment_id: segment.id,
        segment_title: segment.title.clone(),
        years: segment_yearly_bests_from_models(efforts, &activities_by_id),
    })
}

fn segment_yearly_bests_from_models(
    efforts: Vec<segment_efforts::Model>,
    activities_by_id: &HashMap<i32, EffortActivitySummaryRow>,
) -> Vec<SegmentYearlyBestResponse> {
    let mut best_by_year = HashMap::<i32, SegmentYearlyBestResponse>::new();

    for effort in efforts {
        let Some(activity) = activities_by_id.get(&effort.activity_id) else {
            continue;
        };
        let year = activity.started_at.year();
        let candidate = SegmentYearlyBestResponse {
            year,
            effort_id: effort.id,
            activity_id: effort.activity_id,
            activity_title: activity.title.clone(),
            activity_started_at: activity.started_at,
            effort_index: effort.effort_index,
            duration_seconds: effort.duration_seconds,
            improvement_from_previous_year_seconds: None,
            improvement_from_first_year_seconds: None,
        };

        let should_replace = best_by_year.get(&year).is_none_or(|current| {
            (candidate.duration_seconds, candidate.effort_id)
                < (current.duration_seconds, current.effort_id)
        });

        if should_replace {
            best_by_year.insert(year, candidate);
        }
    }

    let mut yearly_bests = best_by_year.into_values().collect::<Vec<_>>();
    yearly_bests.sort_by_key(|best| best.year);

    let first_duration = yearly_bests.first().map(|best| best.duration_seconds);
    let mut previous_duration = None::<i32>;

    for best in &mut yearly_bests {
        best.improvement_from_previous_year_seconds =
            previous_duration.map(|duration| duration - best.duration_seconds);
        best.improvement_from_first_year_seconds =
            first_duration.map(|duration| duration - best.duration_seconds);
        previous_duration = Some(best.duration_seconds);
    }

    yearly_bests
}

async fn load_segment_effort_analysis_response(
    db: &sea_orm::DatabaseConnection,
    segment: &segments::Model,
    user_id: i32,
    query: SegmentEffortAnalysisQuery,
) -> Result<SegmentEffortAnalysisResponse, AppError> {
    let split_count = normalized_analysis_split_count(query.split_count)?;
    let efforts = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::SegmentId.eq(segment.id))
        .filter(segment_efforts::Column::UserId.eq(user_id))
        .order_by_asc(segment_efforts::Column::DurationSeconds)
        .order_by_asc(segment_efforts::Column::Id)
        .all(db)
        .await?;

    if efforts.is_empty() {
        return Err(AppError::not_found("No efforts found for this segment"));
    }

    let activity_ids = efforts
        .iter()
        .map(|effort| effort.activity_id)
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
    let activities_by_id = activity_models
        .into_iter()
        .map(|activity| (activity.id, activity))
        .collect::<HashMap<_, _>>();
    let effort_sources = efforts
        .into_iter()
        .filter_map(|effort| {
            let activity = activities_by_id.get(&effort.activity_id)?;
            let derived_data =
                deserialize_derived_activity_data(activity.derived_data_json.as_ref());
            let route_points = slice_effort_route_points(
                &derived_data.route_points,
                effort.start_route_point_index,
                effort.end_route_point_index,
            );

            if route_points.len() < 2 {
                return None;
            }

            Some(SegmentAnalysisEffortSource {
                effort_id: effort.id,
                activity_id: effort.activity_id,
                activity_title: activity.title.clone(),
                activity_started_at: activity.started_at,
                effort_index: effort.effort_index,
                duration_seconds: effort.duration_seconds,
                route_points,
            })
        })
        .collect::<Vec<_>>();

    segment_effort_analysis_from_sources(
        segment.id,
        segment.title.clone(),
        segment_route_point_responses(&deserialize_segment_route_points(
            segment.route_data_json.as_ref(),
        )),
        effort_sources,
        query.reference_effort_id,
        split_count,
    )
}

fn normalized_analysis_split_count(value: Option<usize>) -> Result<usize, AppError> {
    let split_count = value.unwrap_or(DEFAULT_ANALYSIS_SPLIT_COUNT);

    if !(MIN_ANALYSIS_SPLIT_COUNT..=MAX_ANALYSIS_SPLIT_COUNT).contains(&split_count) {
        return Err(AppError::validation_field(
            "split_count",
            format!(
                "Split count must be between {MIN_ANALYSIS_SPLIT_COUNT} and {MAX_ANALYSIS_SPLIT_COUNT}"
            ),
        ));
    }

    Ok(split_count)
}

fn segment_effort_analysis_from_sources(
    segment_id: i32,
    segment_title: String,
    route_points: Vec<SegmentRoutePointResponse>,
    mut efforts: Vec<SegmentAnalysisEffortSource>,
    reference_effort_id: Option<i32>,
    split_count: usize,
) -> Result<SegmentEffortAnalysisResponse, AppError> {
    efforts.sort_by_key(|effort| (effort.duration_seconds, effort.effort_id));
    let reference_index = match reference_effort_id {
        Some(effort_id) => efforts
            .iter()
            .position(|effort| effort.effort_id == effort_id)
            .ok_or_else(|| AppError::not_found("Reference effort not found"))?,
        None => 0,
    };
    let reference_effort = efforts
        .get(reference_index)
        .cloned()
        .ok_or_else(|| AppError::not_found("No analyzable efforts found for this segment"))?;
    let sampled_efforts = efforts
        .iter()
        .filter_map(|effort| {
            let samples = analysis_samples_for_effort(effort, split_count)?;
            Some((effort, samples))
        })
        .collect::<Vec<_>>();
    let reference_samples = sampled_efforts
        .iter()
        .find(|(effort, _)| effort.effort_id == reference_effort.effort_id)
        .map(|(_, samples)| samples)
        .ok_or_else(|| AppError::not_found("Reference effort is not analyzable"))?;

    let summaries = sampled_efforts
        .iter()
        .map(|(effort, _)| SegmentAnalysisEffortSummaryResponse {
            effort_id: effort.effort_id,
            activity_id: effort.activity_id,
            activity_title: effort.activity_title.clone(),
            activity_started_at: effort.activity_started_at,
            effort_index: effort.effort_index,
            duration_seconds: effort.duration_seconds,
            delta_from_reference_seconds: round_seconds(
                effort.duration_seconds as f64 - reference_effort.duration_seconds as f64,
            ),
        })
        .collect::<Vec<_>>();

    let mut sections = Vec::with_capacity(split_count);
    let mut theoretical_best_duration_seconds = 0.0;

    for section_index in 0..split_count {
        let reference_split_seconds =
            split_seconds(reference_samples, section_index).unwrap_or_default();
        let mut section_efforts = Vec::new();
        let mut best_split: Option<(&SegmentAnalysisEffortSource, f64)> = None;

        for (effort, samples) in &sampled_efforts {
            let Some(split_seconds) = split_seconds(samples, section_index) else {
                continue;
            };
            let average_speed_mps = section_distance_meters(effort, split_count)
                .and_then(|distance| (split_seconds > 0.0).then_some(distance / split_seconds));

            if best_split
                .as_ref()
                .is_none_or(|(best_effort, best_seconds)| {
                    (split_seconds, effort.effort_id) < (*best_seconds, best_effort.effort_id)
                })
            {
                best_split = Some((effort, split_seconds));
            }

            section_efforts.push(SegmentAnalysisSectionEffortResponse {
                effort_id: effort.effort_id,
                activity_id: effort.activity_id,
                activity_title: effort.activity_title.clone(),
                activity_started_at: effort.activity_started_at,
                split_seconds: round_seconds(split_seconds),
                delta_from_reference_seconds: round_seconds(
                    split_seconds - reference_split_seconds,
                ),
                delta_from_best_seconds: 0.0,
                average_speed_mps: average_speed_mps.map(round_metric),
            });
        }

        let Some((best_effort, best_split_seconds)) = best_split else {
            continue;
        };

        for effort in &mut section_efforts {
            effort.delta_from_best_seconds =
                round_seconds(effort.split_seconds - best_split_seconds);
        }
        let mut top_efforts = section_efforts.clone();
        top_efforts.sort_by(|left, right| {
            left.split_seconds
                .total_cmp(&right.split_seconds)
                .then_with(|| left.effort_id.cmp(&right.effort_id))
        });
        top_efforts.truncate(TOP_ANALYSIS_SECTION_EFFORT_COUNT);

        theoretical_best_duration_seconds += best_split_seconds;
        sections.push(SegmentAnalysisSectionResponse {
            section_index: section_index + 1,
            start_progress_percent: round_metric(section_index as f64 * 100.0 / split_count as f64),
            end_progress_percent: round_metric(
                (section_index + 1) as f64 * 100.0 / split_count as f64,
            ),
            reference_split_seconds: round_seconds(reference_split_seconds),
            best_split_seconds: round_seconds(best_split_seconds),
            best_effort_id: best_effort.effort_id,
            best_activity_id: best_effort.activity_id,
            best_activity_title: best_effort.activity_title.clone(),
            gain_available_seconds: round_seconds(reference_split_seconds - best_split_seconds),
            top_efforts,
            efforts: section_efforts,
        });
    }

    let theoretical_best_duration_seconds = round_seconds(theoretical_best_duration_seconds);

    Ok(SegmentEffortAnalysisResponse {
        segment_id,
        segment_title,
        split_count,
        route_points,
        reference_effort: SegmentAnalysisEffortSummaryResponse {
            effort_id: reference_effort.effort_id,
            activity_id: reference_effort.activity_id,
            activity_title: reference_effort.activity_title,
            activity_started_at: reference_effort.activity_started_at,
            effort_index: reference_effort.effort_index,
            duration_seconds: reference_effort.duration_seconds,
            delta_from_reference_seconds: 0.0,
        },
        efforts: summaries,
        sections,
        theoretical_best_duration_seconds,
        theoretical_best_gain_seconds: round_seconds(
            reference_effort.duration_seconds as f64 - theoretical_best_duration_seconds,
        ),
    })
}

fn analysis_samples_for_effort(
    effort: &SegmentAnalysisEffortSource,
    split_count: usize,
) -> Option<Vec<SegmentAnalysisSample>> {
    (0..=split_count)
        .map(|index| {
            let progress = index as f64 / split_count as f64;
            interpolate_activity_route_point_by_progress(&effort.route_points, progress).map(
                |point| SegmentAnalysisSample {
                    elapsed_seconds: point.elapsed_seconds as f64,
                },
            )
        })
        .collect()
}

fn split_seconds(samples: &[SegmentAnalysisSample], section_index: usize) -> Option<f64> {
    let start = samples.get(section_index)?;
    let end = samples.get(section_index + 1)?;
    Some((end.elapsed_seconds - start.elapsed_seconds).max(0.0))
}

fn section_distance_meters(
    effort: &SegmentAnalysisEffortSource,
    split_count: usize,
) -> Option<f64> {
    let total = effort
        .route_points
        .last()
        .and_then(|point| point.distance_meters)?;
    (total > 0.0).then_some(total / split_count as f64)
}

fn interpolate_activity_route_point_by_progress(
    points: &[ActivityRoutePoint],
    progress: f64,
) -> Option<ActivityRoutePoint> {
    if points.is_empty() {
        return None;
    }

    if points.len() == 1 {
        return points.first().cloned();
    }

    let clamped_progress = progress.clamp(0.0, 1.0);
    if clamped_progress <= 0.0 {
        return points.first().cloned();
    }

    let distance_range = activity_route_distance_range(points);
    let target_measure = distance_range
        .map(|(_, total)| clamped_progress * total)
        .unwrap_or_else(|| clamped_progress * (points.len() - 1) as f64);

    for index in 1..points.len() {
        let previous = &points[index - 1];
        let current = &points[index];
        let previous_measure = distance_range
            .map(|(first, _)| previous.distance_meters.unwrap_or(first) - first)
            .unwrap_or((index - 1) as f64);
        let current_measure = distance_range
            .map(|(first, _)| current.distance_meters.unwrap_or(first) - first)
            .unwrap_or(index as f64);

        if target_measure <= current_measure {
            let span = (current_measure - previous_measure).max(f64::EPSILON);
            let local_progress = (target_measure - previous_measure) / span;
            return Some(interpolate_activity_route_point(
                previous,
                current,
                local_progress,
                distance_range.map(|_| target_measure),
            ));
        }
    }

    points.last().cloned()
}

fn activity_route_distance_range(points: &[ActivityRoutePoint]) -> Option<(f64, f64)> {
    let first_distance = points.first()?.distance_meters?;
    let last_distance = points.last()?.distance_meters?;

    if last_distance <= first_distance
        || !points
            .iter()
            .all(|point| point.distance_meters.is_some_and(f64::is_finite))
    {
        return None;
    }

    Some((first_distance, last_distance - first_distance))
}

fn interpolate_activity_route_point(
    previous: &ActivityRoutePoint,
    current: &ActivityRoutePoint,
    progress: f64,
    normalized_distance_meters: Option<f64>,
) -> ActivityRoutePoint {
    ActivityRoutePoint {
        elapsed_seconds: interpolate_i32(
            previous.elapsed_seconds,
            current.elapsed_seconds,
            progress,
        ),
        latitude: interpolate_f64(previous.latitude, current.latitude, progress),
        longitude: interpolate_f64(previous.longitude, current.longitude, progress),
        distance_meters: normalized_distance_meters.or_else(|| {
            interpolate_optional_f64(previous.distance_meters, current.distance_meters, progress)
        }),
        elevation_meters: interpolate_optional_f64(
            previous.elevation_meters,
            current.elevation_meters,
            progress,
        ),
        speed_mps: interpolate_optional_f64(previous.speed_mps, current.speed_mps, progress),
        heart_rate_bpm: interpolate_optional_i32(
            previous.heart_rate_bpm,
            current.heart_rate_bpm,
            progress,
        ),
        cadence_rpm: interpolate_optional_i32(previous.cadence_rpm, current.cadence_rpm, progress),
        power_watts: interpolate_optional_i32(previous.power_watts, current.power_watts, progress),
    }
}

fn interpolate_f64(previous: f64, current: f64, progress: f64) -> f64 {
    previous + (current - previous) * progress
}

fn interpolate_i32(previous: i32, current: i32, progress: f64) -> i32 {
    interpolate_f64(previous as f64, current as f64, progress).round() as i32
}

fn interpolate_optional_f64(
    previous: Option<f64>,
    current: Option<f64>,
    progress: f64,
) -> Option<f64> {
    match (previous, current) {
        (Some(previous), Some(current)) => Some(interpolate_f64(previous, current, progress)),
        (Some(previous), None) => Some(previous),
        (None, Some(current)) => Some(current),
        (None, None) => None,
    }
}

fn interpolate_optional_i32(
    previous: Option<i32>,
    current: Option<i32>,
    progress: f64,
) -> Option<i32> {
    match (previous, current) {
        (Some(previous), Some(current)) => Some(interpolate_i32(previous, current, progress)),
        (Some(previous), None) => Some(previous),
        (None, Some(current)) => Some(current),
        (None, None) => None,
    }
}

fn round_seconds(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round_metric(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
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

    fn build_effort_model(
        id: i32,
        activity_id: i32,
        duration_seconds: i32,
    ) -> segment_efforts::Model {
        let now = Utc::now();

        segment_efforts::Model {
            id,
            segment_id: 10,
            user_id: 7,
            activity_id,
            effort_index: id,
            duration_seconds,
            start_elapsed_seconds: 0,
            end_elapsed_seconds: duration_seconds,
            start_route_point_index: 0,
            end_route_point_index: 1,
            distance_meters: Some(1800.0),
            overall_rank: None,
            user_rank: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn build_effort_activity_row(
        id: i32,
        title: &str,
        started_at: DateTime<Utc>,
    ) -> EffortActivitySummaryRow {
        EffortActivitySummaryRow {
            id,
            title: title.to_string(),
            started_at,
        }
    }

    fn build_analysis_effort(
        effort_id: i32,
        activity_id: i32,
        title: &str,
        duration_seconds: i32,
        elapsed_boundaries: &[i32],
    ) -> SegmentAnalysisEffortSource {
        let route_points = elapsed_boundaries
            .iter()
            .enumerate()
            .map(|(index, elapsed_seconds)| {
                let distance_meters = index as f64 * 100.0;
                ActivityRoutePoint {
                    elapsed_seconds: *elapsed_seconds,
                    latitude: 44.0 + index as f64 * 0.001,
                    longitude: -93.0 - index as f64 * 0.001,
                    distance_meters: Some(distance_meters),
                    elevation_meters: None,
                    speed_mps: Some(100.0 / elapsed_seconds.max(&1).to_owned() as f64),
                    heart_rate_bpm: None,
                    cadence_rpm: None,
                    power_watts: None,
                }
            })
            .collect::<Vec<_>>();

        SegmentAnalysisEffortSource {
            effort_id,
            activity_id,
            activity_title: title.to_string(),
            activity_started_at: DateTime::parse_from_rfc3339("2026-04-01T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            effort_index: effort_id,
            duration_seconds,
            route_points,
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
    fn segment_yearly_bests_choose_fastest_effort_per_activity_year() {
        let activities_by_id = HashMap::from([
            (
                100,
                build_effort_activity_row(
                    100,
                    "Spring ride",
                    DateTime::parse_from_rfc3339("2024-04-01T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                ),
            ),
            (
                101,
                build_effort_activity_row(
                    101,
                    "Summer ride",
                    DateTime::parse_from_rfc3339("2024-07-01T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                ),
            ),
            (
                102,
                build_effort_activity_row(
                    102,
                    "Next year ride",
                    DateTime::parse_from_rfc3339("2025-05-01T12:00:00Z")
                        .unwrap()
                        .with_timezone(&Utc),
                ),
            ),
        ]);
        let efforts = vec![
            build_effort_model(1, 100, 90),
            build_effort_model(2, 101, 82),
            build_effort_model(3, 102, 76),
        ];

        let yearly_bests = segment_yearly_bests_from_models(efforts, &activities_by_id);

        assert_eq!(yearly_bests.len(), 2);
        assert_eq!(yearly_bests[0].year, 2024);
        assert_eq!(yearly_bests[0].effort_id, 2);
        assert_eq!(yearly_bests[0].duration_seconds, 82);
        assert_eq!(yearly_bests[0].improvement_from_first_year_seconds, Some(0));
        assert_eq!(yearly_bests[1].year, 2025);
        assert_eq!(yearly_bests[1].duration_seconds, 76);
        assert_eq!(
            yearly_bests[1].improvement_from_previous_year_seconds,
            Some(6)
        );
        assert_eq!(yearly_bests[1].improvement_from_first_year_seconds, Some(6));
    }

    #[test]
    fn segment_yearly_bests_ignore_missing_activity_rows_and_tie_break_by_effort_id() {
        let activities_by_id = HashMap::from([(
            100,
            build_effort_activity_row(
                100,
                "Tie ride",
                DateTime::parse_from_rfc3339("2026-04-01T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            ),
        )]);
        let efforts = vec![
            build_effort_model(5, 100, 65),
            build_effort_model(4, 100, 65),
            build_effort_model(3, 999, 50),
        ];

        let yearly_bests = segment_yearly_bests_from_models(efforts, &activities_by_id);

        assert_eq!(yearly_bests.len(), 1);
        assert_eq!(yearly_bests[0].effort_id, 4);
        assert_eq!(yearly_bests[0].duration_seconds, 65);
        assert_eq!(yearly_bests[0].improvement_from_previous_year_seconds, None);
        assert_eq!(yearly_bests[0].improvement_from_first_year_seconds, Some(0));
    }

    #[test]
    fn effort_analysis_uses_reference_and_stitches_best_splits() {
        let efforts = vec![
            build_analysis_effort(1, 101, "PR", 100, &[0, 30, 60, 100]),
            build_analysis_effort(2, 102, "Fast top", 103, &[0, 25, 65, 103]),
            build_analysis_effort(3, 103, "Fast middle", 105, &[0, 35, 58, 105]),
        ];

        let analysis = segment_effort_analysis_from_sources(
            10,
            "Breaking the Law".to_string(),
            Vec::new(),
            efforts,
            None,
            3,
        )
        .unwrap();

        assert_eq!(analysis.reference_effort.effort_id, 1);
        assert_eq!(analysis.sections.len(), 3);
        assert_eq!(analysis.sections[0].best_effort_id, 2);
        assert_eq!(analysis.sections[0].best_split_seconds, 25.0);
        assert_eq!(
            analysis.sections[0]
                .top_efforts
                .iter()
                .map(|effort| effort.effort_id)
                .collect::<Vec<_>>(),
            vec![2, 1, 3]
        );
        assert_eq!(analysis.sections[1].best_effort_id, 3);
        assert_eq!(analysis.sections[1].best_split_seconds, 23.0);
        assert_eq!(analysis.sections[2].best_effort_id, 2);
        assert_eq!(analysis.theoretical_best_duration_seconds, 86.0);
        assert_eq!(analysis.theoretical_best_gain_seconds, 14.0);
    }

    #[test]
    fn effort_analysis_can_use_requested_reference_effort() {
        let efforts = vec![
            build_analysis_effort(1, 101, "PR", 100, &[0, 30, 60, 100]),
            build_analysis_effort(2, 102, "Other", 103, &[0, 25, 65, 103]),
        ];

        let analysis = segment_effort_analysis_from_sources(
            10,
            "Breaking the Law".to_string(),
            Vec::new(),
            efforts,
            Some(2),
            3,
        )
        .unwrap();

        assert_eq!(analysis.reference_effort.effort_id, 2);
        assert_eq!(analysis.efforts[0].delta_from_reference_seconds, -3.0);
        assert_eq!(analysis.efforts[1].delta_from_reference_seconds, 0.0);
    }

    #[test]
    fn normalized_analysis_split_count_rejects_out_of_range_values() {
        assert_eq!(normalized_analysis_split_count(None).unwrap(), 10);
        assert_eq!(normalized_analysis_split_count(Some(2)).unwrap(), 2);
        assert_eq!(normalized_analysis_split_count(Some(30)).unwrap(), 30);
        assert!(normalized_analysis_split_count(Some(1)).is_err());
        assert!(normalized_analysis_split_count(Some(31)).is_err());
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
