use crate::activity_import_lock::{
    acquire_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING, ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
    ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP,
    ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION, ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::app_error::{ApiErrorResponse, AppError};
use crate::controllers::activity_import_history;
use crate::controllers::activity_imports as activity_imports_controller;
use crate::storage::AppStorage;
use crate::xc_goal_backfill::queue_user_xc_goal_backfill;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use bike_core::activity_import_pipeline::ACTIVITY_PROCESSING_PROVIDER;
use bike_core::activity_lifecycle::cleanup_duplicate_activities_for_user;
use bike_core::analytics::mark_user_activity_changes;
use bike_core::archive_import::{
    import_activity_archive_from_path, resolve_local_archive_import_path,
    ImportActivityArchiveRequest,
};
use bike_core::auth::entities::users;
use bike_core::auth::AdminUserContext;
use bike_core::entities::{activities, activity_imports, segments};
use bike_core::integration_events_service as integration_event_service;
use bike_core::platform::data::pagination::{PaginatedResponse, PaginationParams};
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const SEGMENT_BACKFILL_CHUNK_SIZE: usize = 250;

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 42,"user_id": 12,"title": "Saturday hill repeats","sport": "ride","source": "manual_upload","started_at": "2026-09-26T13:00:00Z","activity_import_id": 17,"import_status": "processed","import_processing_stage": "complete"}))]
pub struct AdminActivityResponse {
    #[schema(example = 42)]
    pub id: i32,
    #[schema(example = 12)]
    pub user_id: i32,
    #[schema(example = "Saturday hill repeats")]
    pub title: String,
    #[schema(example = "ride")]
    pub sport: String,
    #[schema(example = "manual_upload")]
    pub source: String,
    #[schema(example = "2026-09-26T13:00:00Z")]
    pub started_at: chrono::DateTime<Utc>,
    pub format: Option<String>,
    #[schema(example = 17)]
    pub activity_import_id: Option<i32>,
    pub import_version: Option<i32>,
    #[schema(example = "processed")]
    pub import_status: Option<String>,
    #[schema(example = "complete")]
    pub import_processing_stage: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"trace": null}))]
pub struct AdminActivityImportTraceResponse {
    #[schema(example = json!(null))]
    pub trace: Option<activity_imports_controller::ActivityImportTraceResponse>,
}

impl AdminActivityResponse {
    fn from_list_row(
        activity: activities::AdminListRow,
        import: Option<&activity_imports::Model>,
    ) -> Self {
        Self {
            id: activity.id,
            user_id: activity.user_id,
            title: activity.title,
            sport: activity.sport,
            source: activity.source,
            started_at: activity.started_at,
            format: activity.format,
            activity_import_id: activity.activity_import_id,
            import_version: import.map(|value| value.import_version),
            import_status: import.map(|value| value.status.clone()),
            import_processing_stage: import.map(|value| value.processing_stage.clone()),
        }
    }
}

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route(
            "/activity-imports/history",
            get(activity_import_history::history),
        )
        .route(
            "/activity-imports/:id/replay",
            get(activity_import_history::replay_plan).post(activity_import_history::replay),
        )
        .route("/activities", get(list_admin_activities))
        .route("/analytics/backfill", post(backfill_analytics))
        .route("/training/xc-backfill", post(backfill_user_xc_training))
        .route("/segments/regenerate", post(regenerate_user_segments))
        .route(
            "/segments/regenerate-efforts",
            post(regenerate_segment_efforts),
        )
        .route(
            "/activity-imports/reprocess",
            post(reprocess_user_activity_imports),
        )
        .route(
            "/activity-imports/:id/trace",
            get(get_admin_activity_import_trace),
        )
        .route(
            "/activities/:id/import-trace",
            get(get_admin_activity_import_trace_for_activity),
        )
        .route(
            "/activity-imports/reprocess-activity",
            post(reprocess_activity_import),
        )
        .route(
            "/activity-imports/cleanup-duplicates",
            post(cleanup_user_duplicate_activities),
        )
        .route("/activity-imports/archive", post(import_activity_archive))
}

#[utoipa::path(
    get,
    path = "/admin/activities",
    operation_id = "admin_list_activities",
    params(PaginationParams, activities::AdminListFilters),
    responses(
        (status = 200, description = "Paginated activities for administrators", body = PaginatedResponse<AdminActivityResponse>, example = json!({"data": [{"id": 42,"user_id": 12,"title": "Saturday hill repeats","sport": "ride","source": "manual_upload","started_at": "2026-09-26T13:00:00Z"}],"metadata": {"page": 1,"per_page": 20,"total": 1,"total_pages": 1}})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 400, description = "Invalid filters", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn list_admin_activities(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(params): Query<PaginationParams>,
    Query(filters): Query<activities::AdminListFilters>,
) -> Result<Json<PaginatedResponse<AdminActivityResponse>>, AppError> {
    if filters.user_id.is_some_and(|id| id <= 0) {
        return Err(AppError::bad_request("User ID must be positive"));
    }
    let (page, per_page) = params.normalized();
    let (activities, total) =
        activities::Model::admin_page(&state.db, &filters, page as u64, per_page as u64).await?;
    let import_ids = activities
        .iter()
        .filter_map(|activity| activity.activity_import_id)
        .collect::<Vec<_>>();
    let imports_by_id = if import_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        activity_imports::Entity::find()
            .filter(activity_imports::Column::Id.is_in(import_ids))
            .all(&state.db)
            .await?
            .into_iter()
            .map(|import| (import.id, import))
            .collect::<std::collections::HashMap<_, _>>()
    };

    Ok(Json(PaginatedResponse::new(
        activities
            .into_iter()
            .map(|activity| {
                let import = activity
                    .activity_import_id
                    .and_then(|import_id| imports_by_id.get(&import_id));
                AdminActivityResponse::from_list_row(activity, import)
            })
            .collect(),
        page,
        per_page,
        total as i64,
    )))
}

#[utoipa::path(
    get,
    path = "/admin/activity-imports/{id}/trace",
    operation_id = "admin_get_activity_import_trace",
    params(
        ("id" = i32, Path, description = "Activity import id", example = 17),
    ),
    responses(
        (status = 200, description = "Activity import processing DAG and event trace for administrators", body = activity_imports_controller::ActivityImportTraceResponse, example = json!({"import": {"id": 17,"import_version": 1,"original_filename": "saturday-hills.fit","format": "fit","status": "processed","processing_stage": "complete","size_bytes": 184320,"created_at": "2026-09-26T14:20:00Z"},"graph": {"nodes": [{"id": "raw_stored","label": "Raw stored","stage": "raw_stored"}],"edges": [],"mermaid": "flowchart LR\n  raw_stored[\"Raw stored\"]"},"nodes": [{"id": "raw_stored","label": "Raw stored","stage": "raw_stored","status": "completed"}],"events": []})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Activity import not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn get_admin_activity_import_trace(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(import_id): Path<i32>,
) -> Result<Json<activity_imports_controller::ActivityImportTraceResponse>, AppError> {
    let import =
        crate::activity_import_history::admin_import_for_inspection(&state.db, import_id).await?;
    let trace = build_admin_activity_import_trace(&state.db, import, None).await?;

    Ok(Json(trace))
}

#[utoipa::path(
    get,
    path = "/admin/activities/{id}/import-trace",
    operation_id = "admin_get_activity_import_trace_for_activity",
    params(
        ("id" = i32, Path, description = "Activity id", example = 42),
    ),
    responses(
        (status = 200, description = "Activity import processing DAG and event trace for administrators when the activity has a linked import", body = AdminActivityImportTraceResponse, example = json!({"trace": null})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn get_admin_activity_import_trace_for_activity(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(activity_id): Path<i32>,
) -> Result<Json<AdminActivityImportTraceResponse>, AppError> {
    let activity = activities::Entity::find_by_id(activity_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let Some(import_id) = activity.activity_import_id else {
        return Ok(Json(AdminActivityImportTraceResponse { trace: None }));
    };
    let import = activity_imports::Entity::find_by_id(import_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity import not found"))?;
    let trace = build_admin_activity_import_trace(&state.db, import, Some(activity)).await?;

    Ok(Json(AdminActivityImportTraceResponse {
        trace: Some(trace),
    }))
}

async fn build_admin_activity_import_trace(
    db: &DatabaseConnection,
    import: activity_imports::Model,
    activity: Option<activities::Model>,
) -> Result<activity_imports_controller::ActivityImportTraceResponse, AppError> {
    let activity = match activity {
        Some(activity) => Some(activity),
        None => load_activity_for_admin_import_trace(db, &import).await?,
    };
    let attempts = crate::activity_import_history::attempts(db, &import).await?;
    let events = if attempts.is_empty() {
        integration_event_service::list_recent_events(
            db,
            integration_event_service::IntegrationEventListOptions {
                provider: Some(ACTIVITY_PROCESSING_PROVIDER.to_string()),
                user_id: Some(import.user_id),
                activity_id: None,
                import_id: Some(import.id),
                limit: 100,
            },
        )
        .await?
    } else {
        Vec::new()
    };
    let trace_events = events
        .into_iter()
        .map(activity_imports_controller::ActivityImportTraceEventResponse::from_model)
        .collect::<Vec<_>>();
    let trace_nodes = attempts
        .first()
        .map(|attempt| attempt.nodes.clone())
        .unwrap_or(activity_imports_controller::build_trace_nodes(
            &import,
            &trace_events,
        )?);

    Ok(activity_imports_controller::ActivityImportTraceResponse {
        attempts,
        import: activity_imports_controller::ActivityImportResponse::from_model(
            import,
            activity.as_ref(),
        ),
        graph: activity_imports_controller::ActivityProcessingGraphResponse::from_graph(),
        nodes: trace_nodes,
        events: trace_events,
    })
}

async fn load_activity_for_admin_import_trace(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
) -> Result<Option<activities::Model>, AppError> {
    if let Some(activity_id) = import.activity_id {
        return activities::Entity::find_by_id(activity_id)
            .one(db)
            .await
            .map_err(AppError::from);
    }

    activities::Entity::find()
        .filter(activities::Column::ActivityImportId.eq(Some(import.id)))
        .one(db)
        .await
        .map_err(AppError::from)
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"user_count": 12,"segment_count": 35,"fitness_task_count": 12,"segment_task_count": 2,"total_tasks_enqueued": 14,"segment_chunk_size": 20}))]
pub struct AnalyticsBackfillResponse {
    #[schema(example = 12)]
    pub user_count: i32,
    #[schema(example = 35)]
    pub segment_count: i32,
    #[schema(example = 12)]
    pub fitness_task_count: i32,
    #[schema(example = 2)]
    pub segment_task_count: i32,
    #[schema(example = 14)]
    pub total_tasks_enqueued: i32,
    #[schema(example = 20)]
    pub segment_chunk_size: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"archive_path": "/data/imports/activities.zip"}))]
pub struct ArchiveImportRequest {
    #[schema(example = "/data/imports/activities.zip")]
    pub archive_path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"user_id": 12}))]
pub struct RegenerateUserSegmentsRequest {
    #[schema(example = 12)]
    pub user_id: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"segment_id": 7}))]
pub struct RegenerateSegmentEffortsRequest {
    #[schema(example = 7)]
    pub segment_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"archive_path": "/data/imports/activities.zip","total_entries": 120,"supported_entry_count": 116,"imported_count": 110,"duplicate_count": 6,"skipped_unsupported_count": 4,"failed_count": 0,"error_samples": []}))]
pub struct ArchiveImportResponse {
    #[schema(example = "/data/imports/activities.zip")]
    pub archive_path: String,
    #[schema(example = 120)]
    pub total_entries: i32,
    #[schema(example = 116)]
    pub supported_entry_count: i32,
    #[schema(example = 110)]
    pub imported_count: i32,
    #[schema(example = 6)]
    pub duplicate_count: i32,
    #[schema(example = 4)]
    pub skipped_unsupported_count: i32,
    #[schema(example = 0)]
    pub failed_count: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schema(example = json!([]))]
    pub error_samples: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"user_id": 12,"status": "queued","message": "Segment regeneration queued."}))]
pub struct RegenerateUserSegmentsResponse {
    #[schema(example = 12)]
    pub user_id: i32,
    #[schema(example = "queued")]
    pub status: String,
    #[schema(example = "Segment regeneration queued.")]
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"segment_id": 7,"status": "queued","message": "Segment effort regeneration queued.","task_id": "43","task_status": "pending"}))]
pub struct RegenerateSegmentEffortsResponse {
    #[schema(example = 7)]
    pub segment_id: i32,
    #[schema(example = "queued")]
    pub status: String,
    #[schema(example = "Segment effort regeneration queued.")]
    pub message: String,
    #[schema(example = "43")]
    pub task_id: String,
    #[schema(example = "pending")]
    pub task_status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"user_id": 12,"archive_fits_only": true}))]
pub struct ReprocessUserActivityImportsRequest {
    #[schema(example = 12)]
    pub user_id: i32,
    #[serde(default)]
    #[schema(example = true)]
    pub archive_fits_only: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"user_id": 12}))]
pub struct BackfillUserXcTrainingRequest {
    #[schema(example = 12)]
    pub user_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"user_id": 12,"status": "queued","message": "Activity reprocessing queued."}))]
pub struct ReprocessUserActivityImportsResponse {
    #[schema(example = 12)]
    pub user_id: i32,
    #[schema(example = "queued")]
    pub status: String,
    #[schema(example = "Activity reprocessing queued.")]
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"activity_id": 42}))]
pub struct ReprocessActivityImportRequest {
    #[schema(example = 42)]
    pub activity_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"activity_id": 42,"activity_import_id": 17,"user_id": 12,"status": "queued","message": "Activity reprocessing queued.","task_id": "42","task_status": "pending"}))]
pub struct ReprocessActivityImportResponse {
    #[schema(example = 42)]
    pub activity_id: i32,
    #[schema(example = 17)]
    pub activity_import_id: i32,
    #[schema(example = 12)]
    pub user_id: i32,
    #[schema(example = "queued")]
    pub status: String,
    #[schema(example = "Activity reprocessing queued.")]
    pub message: String,
    #[schema(example = "42")]
    pub task_id: String,
    #[schema(example = "pending")]
    pub task_status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[schema(example = json!({"user_id": 12}))]
pub struct CleanupUserDuplicateActivitiesRequest {
    #[schema(example = 12)]
    pub user_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"user_id": 12,"status": "completed","message": "Removed 2 duplicate activities across 2 duplicate groups.","duplicate_group_count": 2,"deleted_activity_count": 2,"retained_activity_count": 2}))]
pub struct CleanupUserDuplicateActivitiesResponse {
    #[schema(example = 12)]
    pub user_id: i32,
    #[schema(example = "completed")]
    pub status: String,
    #[schema(example = "Removed 2 duplicate activities across 2 duplicate groups.")]
    pub message: String,
    #[schema(example = 2)]
    pub duplicate_group_count: i32,
    #[schema(example = 2)]
    pub deleted_activity_count: i32,
    #[schema(example = 2)]
    pub retained_activity_count: i32,
}

#[utoipa::path(
    post,
    path = "/admin/analytics/backfill",
    operation_id = "admin_backfill_analytics",
    responses(
        (status = 202, description = "Enqueued analytics backfill tasks", body = AnalyticsBackfillResponse, example = json!({"user_count": 12,"segment_count": 35,"fitness_task_count": 12,"segment_task_count": 2,"total_tasks_enqueued": 14,"segment_chunk_size": 20})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn backfill_analytics(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<(StatusCode, Json<AnalyticsBackfillResponse>), AppError> {
    let user_ids = load_user_ids_with_activities(&state).await?;
    let segment_ids = load_segment_ids(&state).await?;
    let changed_at = Utc::now();

    mark_user_activity_changes(&state.db, &user_ids, changed_at).await?;

    for user_id in &user_ids {
        state.tasks.rebuild_fitness_freshness(*user_id).await?;
    }

    let segment_task_count = enqueue_segment_backfill_tasks(&state, &segment_ids).await?;
    let fitness_task_count = user_ids.len() as i32;

    Ok((
        StatusCode::ACCEPTED,
        Json(AnalyticsBackfillResponse {
            user_count: user_ids.len() as i32,
            segment_count: segment_ids.len() as i32,
            fitness_task_count,
            segment_task_count,
            total_tasks_enqueued: fitness_task_count + segment_task_count,
            segment_chunk_size: SEGMENT_BACKFILL_CHUNK_SIZE as i32,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/training/xc-backfill",
    operation_id = "admin_backfill_user_xc_training",
    request_body(content = BackfillUserXcTrainingRequest, example = json!({"user_id": 12})),
    responses(
        (status = 202, description = "Queued XC training backfill for one user", body = ReprocessUserActivityImportsResponse, example = json!({"user_id": 12,"status": "queued","message": "XC training backfill queued. Historical rides will repopulate in the background."})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn backfill_user_xc_training(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<BackfillUserXcTrainingRequest>,
) -> Result<(StatusCode, Json<ReprocessUserActivityImportsResponse>), AppError> {
    users::Entity::find_by_id(request.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;

    let (status, message) =
        queue_user_xc_goal_backfill(&state.db, &state.tasks, request.user_id).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(ReprocessUserActivityImportsResponse {
            user_id: request.user_id,
            status,
            message,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/segments/regenerate",
    operation_id = "admin_regenerate_user_segments",
    request_body(content = RegenerateUserSegmentsRequest, example = json!({"user_id": 12})),
    responses(
        (status = 202, description = "Queued segment regeneration for one user", body = RegenerateUserSegmentsResponse, example = json!({"user_id": 12,"status": "queued","message": "Segment regeneration queued."})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn regenerate_user_segments(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<RegenerateUserSegmentsRequest>,
) -> Result<(StatusCode, Json<RegenerateUserSegmentsResponse>), AppError> {
    users::Entity::find_by_id(request.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;

    acquire_user_activity_import_lock(
        &state.db,
        request.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    )
    .await?;

    if let Err(message) = state.tasks.regenerate_user_segments(request.user_id).await {
        release_user_activity_import_lock(
            &state.db,
            request.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
        )
        .await?;
        return Err(AppError::internal(format!(
            "Failed to queue segment regeneration: {message}"
        )));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(RegenerateUserSegmentsResponse {
            user_id: request.user_id,
            status: "queued".to_string(),
            message: "Segment regeneration queued.".to_string(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/segments/regenerate-efforts",
    operation_id = "admin_regenerate_segment_efforts",
    request_body(content = RegenerateSegmentEffortsRequest, example = json!({"segment_id": 7})),
    responses(
        (status = 202, description = "Queued effort regeneration for one segment", body = RegenerateSegmentEffortsResponse, example = json!({"segment_id": 7,"status": "queued","message": "Segment effort regeneration queued.","task_id": "43","task_status": "pending"})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Segment not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn regenerate_segment_efforts(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<RegenerateSegmentEffortsRequest>,
) -> Result<(StatusCode, Json<RegenerateSegmentEffortsResponse>), AppError> {
    segments::Entity::find_by_id(request.segment_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| {
            AppError::not_found(format!("Segment {} was not found", request.segment_id))
        })?;

    let task = state
        .tasks
        .regenerate_segment_efforts(request.segment_id)
        .await
        .map_err(|message| {
            AppError::internal(format!(
                "Failed to queue segment effort regeneration: {message}"
            ))
        })?;

    Ok((
        StatusCode::ACCEPTED,
        Json(RegenerateSegmentEffortsResponse {
            segment_id: request.segment_id,
            status: "queued".to_string(),
            message: "Segment effort regeneration queued.".to_string(),
            task_id: task.id,
            task_status: task.status,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/activity-imports/reprocess",
    operation_id = "admin_reprocess_user_activity_imports",
    request_body(content = ReprocessUserActivityImportsRequest, example = json!({"user_id": 12,"archive_fits_only": true})),
    responses(
        (status = 202, description = "Queued stored-file activity reprocessing for one user", body = ReprocessUserActivityImportsResponse, example = json!({"user_id": 12,"status": "queued","message": "Activity reprocessing queued."})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn reprocess_user_activity_imports(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<ReprocessUserActivityImportsRequest>,
) -> Result<(StatusCode, Json<ReprocessUserActivityImportsResponse>), AppError> {
    users::Entity::find_by_id(request.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;

    acquire_user_activity_import_lock(
        &state.db,
        request.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    )
    .await?;

    let queue_result = if request.archive_fits_only.unwrap_or(false) {
        state
            .tasks
            .reprocess_archive_fit_activity_imports(request.user_id)
            .await
    } else {
        state
            .tasks
            .reprocess_user_activity_imports(request.user_id)
            .await
    };
    if let Err(message) = queue_result {
        release_user_activity_import_lock(
            &state.db,
            request.user_id,
            ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        )
        .await?;
        return Err(AppError::internal(format!(
            "Failed to queue activity reprocessing: {message}"
        )));
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(ReprocessUserActivityImportsResponse {
            user_id: request.user_id,
            status: "queued".to_string(),
            message: "Activity reprocessing queued.".to_string(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/activity-imports/reprocess-activity",
    operation_id = "admin_reprocess_activity_import",
    request_body(content = ReprocessActivityImportRequest, example = json!({"activity_id": 42})),
    responses(
        (status = 202, description = "Queued stored-file activity reprocessing for one activity", body = ReprocessActivityImportResponse, example = json!({"activity_id": 42,"activity_import_id": 17,"user_id": 12,"status": "queued","message": "Activity reprocessing queued.","task_id": "42","task_status": "pending"})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Activity not found", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn reprocess_activity_import(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<ReprocessActivityImportRequest>,
) -> Result<(StatusCode, Json<ReprocessActivityImportResponse>), AppError> {
    let activity = activities::Entity::find_by_id(request.activity_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| {
            AppError::not_found(format!("Activity {} was not found", request.activity_id))
        })?;

    let Some(activity_import_id) = activity.activity_import_id else {
        return Err(AppError::bad_request(format!(
            "Activity {} is not linked to a stored import",
            request.activity_id
        )));
    };

    acquire_user_activity_import_lock(
        &state.db,
        activity.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    )
    .await?;

    let task = match state.tasks.reprocess_activity_import(activity.id).await {
        Ok(task) => task,
        Err(message) => {
            release_user_activity_import_lock(
                &state.db,
                activity.user_id,
                ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
            )
            .await?;
            return Err(AppError::internal(format!(
                "Failed to queue activity reprocessing: {message}"
            )));
        }
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(ReprocessActivityImportResponse {
            activity_id: activity.id,
            activity_import_id,
            user_id: activity.user_id,
            status: "queued".to_string(),
            message: "Activity reprocessing queued.".to_string(),
            task_id: task.id,
            task_status: task.status,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/admin/activity-imports/cleanup-duplicates",
    operation_id = "admin_cleanup_user_duplicate_activities",
    request_body(content = CleanupUserDuplicateActivitiesRequest, example = json!({"user_id": 12})),
    responses(
        (status = 200, description = "Removed duplicate activities for one user", body = CleanupUserDuplicateActivitiesResponse, example = json!({"user_id": 12,"status": "completed","message": "Removed 2 duplicate activities across 2 duplicate groups.","duplicate_group_count": 2,"deleted_activity_count": 2,"retained_activity_count": 2})),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn cleanup_user_duplicate_activities(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<CleanupUserDuplicateActivitiesRequest>,
) -> Result<(StatusCode, Json<CleanupUserDuplicateActivitiesResponse>), AppError> {
    users::Entity::find_by_id(request.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found(format!("User {} was not found", request.user_id)))?;

    acquire_user_activity_import_lock(
        &state.db,
        request.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = cleanup_duplicate_activities_for_user(
        &state.db,
        &state.uploads_dir,
        &state.tasks,
        request.user_id,
    )
    .await
    .map(|summary| {
        let message = if summary.deleted_activity_count == 0 {
            "No duplicate activities matched the current dedupe rules.".to_string()
        } else {
            format!(
                "Removed {} duplicate activities across {} duplicate groups.",
                summary.deleted_activity_count, summary.duplicate_group_count
            )
        };

        (
            StatusCode::OK,
            Json(CleanupUserDuplicateActivitiesResponse {
                user_id: request.user_id,
                status: "completed".to_string(),
                message,
                duplicate_group_count: summary.duplicate_group_count as i32,
                deleted_activity_count: summary.deleted_activity_count as i32,
                retained_activity_count: summary.retained_activity_count as i32,
            }),
        )
    });

    let release_result = release_user_activity_import_lock(
        &state.db,
        request.user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error.into()),
        (Ok(_), Err(error)) => Err(error),
        (Ok(response), Ok(())) => Ok(response),
    }
}

#[utoipa::path(
    post,
    path = "/admin/activity-imports/archive",
    operation_id = "admin_import_activity_archive",
    request_body(content = ArchiveImportRequest, example = json!({"archive_path": "/data/imports/activities.zip"})),
    responses(
        (status = 200, description = "Imported activities from an archive on the server", body = ArchiveImportResponse, example = json!({"archive_path": "/data/imports/activities.zip","total_entries": 120,"supported_entry_count": 116,"imported_count": 110,"duplicate_count": 6,"skipped_unsupported_count": 4,"failed_count": 0,"error_samples": []})),
        (status = 400, description = "Invalid archive request", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Archive not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn import_activity_archive(
    admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<ArchiveImportRequest>,
) -> Result<(StatusCode, Json<ArchiveImportResponse>), AppError> {
    let archive_path =
        resolve_local_archive_import_path(&state.uploads_dir, &request.archive_path)?;
    let user_storage_key = admin.user.pid.to_string();
    acquire_user_activity_import_lock(
        &state.db,
        admin.user.id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = import_activity_archive_from_path(
        &state.db,
        &state.tasks,
        ImportActivityArchiveRequest {
            archive_job_id: None,
            uploads_dir: &state.uploads_dir,
            user_storage_key: &user_storage_key,
            user_id: admin.user.id,
            activity_source: "archive_import",
            display_source: archive_path.display().to_string(),
            archive_path: &archive_path,
        },
    )
    .await
    .map(|summary| {
        (
            StatusCode::OK,
            Json(ArchiveImportResponse {
                archive_path: summary.source,
                total_entries: summary.total_entries,
                supported_entry_count: summary.supported_entry_count,
                imported_count: summary.imported_count,
                duplicate_count: summary.duplicate_count,
                skipped_unsupported_count: summary.skipped_unsupported_count,
                failed_count: summary.failed_count,
                error_samples: summary.error_samples,
            }),
        )
    });

    let release_result = release_user_activity_import_lock(
        &state.db,
        admin.user.id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error.into()),
        (Ok(_), Err(error)) => Err(error),
        (Ok(response), Ok(())) => Ok(response),
    }
}

async fn load_user_ids_with_activities(state: &Arc<AppStorage>) -> Result<Vec<i32>, AppError> {
    let mut user_ids = activities::Entity::find()
        .select_only()
        .column(activities::Column::UserId)
        .filter(activities::Column::UserId.is_not_null())
        .distinct()
        .into_tuple::<i32>()
        .all(&state.db)
        .await?;
    user_ids.sort_unstable();
    user_ids.dedup();

    Ok(user_ids)
}

async fn load_segment_ids(state: &Arc<AppStorage>) -> Result<Vec<i32>, AppError> {
    let mut segment_ids = segments::Entity::find()
        .select_only()
        .column(segments::Column::Id)
        .into_tuple::<i32>()
        .all(&state.db)
        .await?;
    segment_ids.sort_unstable();
    segment_ids.dedup();

    Ok(segment_ids)
}

async fn enqueue_segment_backfill_tasks(
    state: &Arc<AppStorage>,
    segment_ids: &[i32],
) -> Result<i32, AppError> {
    let mut task_count = 0;

    for chunk in segment_ids.chunks(SEGMENT_BACKFILL_CHUNK_SIZE) {
        state
            .tasks
            .rebuild_segment_analytics(chunk.to_vec())
            .await?;
        task_count += 1;
    }

    Ok(task_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_segment_backfill_enqueues_no_tasks() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");
        let state = Arc::new(AppStorage::for_test(db));

        assert_eq!(
            enqueue_segment_backfill_tasks(&state, &[]).await.unwrap(),
            0
        );
    }
}
