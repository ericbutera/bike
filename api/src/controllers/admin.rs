use crate::activity_import_lock::{
    acquire_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING, ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
    ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP,
    ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION, ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::ACTIVITY_PROCESSING_PROVIDER;
use crate::activity_lifecycle::cleanup_duplicate_activities_for_user;
use crate::analytics::mark_user_activity_changes;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::archive_import::{import_activity_archive_from_path, resolve_local_archive_import_path};
use crate::controllers::activity_imports as activity_imports_controller;
use crate::entities::{activities, activity_imports, segments, strava_connections};
use crate::integration_events as integration_event_service;
use crate::storage::AppStorage;
use crate::xc_goal_backfill::queue_user_xc_goal_backfill;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use kaleido::auth::entities::users;
use kaleido::auth::AdminUserContext;
use kaleido::glass::aggregator::{Aggregator, NamedStat};
use kaleido::glass::data::pagination::{PaginatedResponse, PaginationParams};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const SEGMENT_BACKFILL_CHUNK_SIZE: usize = 250;

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminActivityResponse {
    pub id: i32,
    pub user_id: i32,
    pub title: String,
    pub sport: String,
    pub source: String,
    pub started_at: chrono::DateTime<Utc>,
    pub format: Option<String>,
    pub activity_import_id: Option<i32>,
    pub import_version: Option<i32>,
    pub import_status: Option<String>,
    pub import_processing_stage: Option<String>,
}

#[derive(Debug, FromQueryResult)]
struct AdminActivityListRow {
    id: i32,
    user_id: i32,
    title: String,
    sport: String,
    source: String,
    started_at: DateTime<Utc>,
    format: Option<String>,
    activity_import_id: Option<i32>,
}

impl AdminActivityResponse {
    fn from_list_row(
        activity: AdminActivityListRow,
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
        .route("/metrics/app", get(app_metrics))
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
    path = "/admin/metrics/app",
    operation_id = "admin_app_metrics",
    responses(
        (status = 200, description = "Bike app metrics", body = [NamedStat]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn app_metrics(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<NamedStat>>, AppError> {
    Ok(Json(bike_metrics(&state.db).await))
}

#[utoipa::path(
    get,
    path = "/admin/activities",
    operation_id = "admin_list_activities",
    params(PaginationParams),
    responses(
        (status = 200, description = "Paginated activities for administrators", body = PaginatedResponse<AdminActivityResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn list_admin_activities(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<PaginatedResponse<AdminActivityResponse>>, AppError> {
    let (page, per_page) = params.normalized();
    let total = activities::Entity::find().count(&state.db).await? as i64;
    let activities = activities::Entity::find()
        .select_only()
        .column(activities::Column::Id)
        .column(activities::Column::UserId)
        .column(activities::Column::Title)
        .column(activities::Column::Sport)
        .column(activities::Column::Source)
        .column(activities::Column::StartedAt)
        .column(activities::Column::Format)
        .column(activities::Column::ActivityImportId)
        .order_by_desc(activities::Column::StartedAt)
        .order_by_desc(activities::Column::Id)
        .into_model::<AdminActivityListRow>()
        .paginate(&state.db, per_page as u64)
        .fetch_page((page - 1) as u64)
        .await?;
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
        total,
    )))
}

#[utoipa::path(
    get,
    path = "/admin/activity-imports/{id}/trace",
    operation_id = "admin_get_activity_import_trace",
    params(
        ("id" = i32, Path, description = "Activity import id"),
    ),
    responses(
        (status = 200, description = "Activity import processing DAG and event trace for administrators", body = activity_imports_controller::ActivityImportTraceResponse),
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
    let import = activity_imports::Entity::find_by_id(import_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Activity import not found"))?;
    let activity = load_activity_for_admin_import_trace(&state.db, &import).await?;
    let events = integration_event_service::list_recent_events(
        &state.db,
        integration_event_service::IntegrationEventListOptions {
            provider: Some(ACTIVITY_PROCESSING_PROVIDER.to_string()),
            user_id: Some(import.user_id),
            activity_id: None,
            import_id: Some(import.id),
            limit: 100,
        },
    )
    .await?;
    let trace_events = events
        .into_iter()
        .map(activity_imports_controller::ActivityImportTraceEventResponse::from_model)
        .collect::<Vec<_>>();
    let trace_nodes = activity_imports_controller::build_trace_nodes(&import, &trace_events)?;

    Ok(Json(
        activity_imports_controller::ActivityImportTraceResponse {
            import: activity_imports_controller::ActivityImportResponse::from_model(
                import,
                activity.as_ref(),
            ),
            graph: activity_imports_controller::ActivityProcessingGraphResponse::from_graph(),
            nodes: trace_nodes,
            events: trace_events,
        },
    ))
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

async fn bike_metrics(db: &DatabaseConnection) -> Vec<NamedStat> {
    let total_activities = Aggregator::total::<activities::Entity>(db, activities::Column::Id);
    let activities_added_last_30d =
        Aggregator::recent_count::<activities::Entity>(db, activities::Column::CreatedAt, 30);
    let total_activity_imports =
        Aggregator::total::<activity_imports::Entity>(db, activity_imports::Column::Id);
    let activity_imports_last_30d = Aggregator::recent_count::<activity_imports::Entity>(
        db,
        activity_imports::Column::CreatedAt,
        30,
    );
    let total_segments = Aggregator::total::<segments::Entity>(db, segments::Column::Id);
    let segments_added_last_30d =
        Aggregator::recent_count::<segments::Entity>(db, segments::Column::CreatedAt, 30);
    let total_strava_connections =
        Aggregator::total::<strava_connections::Entity>(db, strava_connections::Column::Id);

    let (
        total_activities,
        activities_added_last_30d,
        total_activity_imports,
        activity_imports_last_30d,
        total_segments,
        segments_added_last_30d,
        total_strava_connections,
    ) = tokio::join!(
        total_activities,
        activities_added_last_30d,
        total_activity_imports,
        activity_imports_last_30d,
        total_segments,
        segments_added_last_30d,
        total_strava_connections,
    );

    vec![
        NamedStat::new(
            "total_activities",
            "Total Activities",
            "all time",
            total_activities,
        ),
        NamedStat::new(
            "activities_added_last_30d",
            "Activities Added (30d)",
            "last 30 days",
            activities_added_last_30d,
        ),
        NamedStat::new(
            "total_activity_imports",
            "Stored Activity Imports",
            "all time",
            total_activity_imports,
        ),
        NamedStat::new(
            "activity_imports_last_30d",
            "Stored Activity Imports (30d)",
            "last 30 days",
            activity_imports_last_30d,
        ),
        NamedStat::new(
            "total_segments",
            "Tracked Segments",
            "all time",
            total_segments,
        ),
        NamedStat::new(
            "segments_added_last_30d",
            "Tracked Segments (30d)",
            "last 30 days",
            segments_added_last_30d,
        ),
        NamedStat::new(
            "total_strava_connections",
            "Connected Strava Accounts",
            "all time",
            total_strava_connections,
        ),
    ]
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AnalyticsBackfillResponse {
    pub user_count: i32,
    pub segment_count: i32,
    pub fitness_task_count: i32,
    pub segment_task_count: i32,
    pub total_tasks_enqueued: i32,
    pub segment_chunk_size: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ArchiveImportRequest {
    pub archive_path: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegenerateUserSegmentsRequest {
    pub user_id: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct RegenerateSegmentEffortsRequest {
    pub segment_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ArchiveImportResponse {
    pub archive_path: String,
    pub total_entries: i32,
    pub supported_entry_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub skipped_unsupported_count: i32,
    pub failed_count: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_samples: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RegenerateUserSegmentsResponse {
    pub user_id: i32,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct RegenerateSegmentEffortsResponse {
    pub segment_id: i32,
    pub status: String,
    pub message: String,
    pub task_id: String,
    pub task_status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReprocessUserActivityImportsRequest {
    pub user_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReprocessUserActivityImportsResponse {
    pub user_id: i32,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReprocessActivityImportRequest {
    pub activity_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ReprocessActivityImportResponse {
    pub activity_id: i32,
    pub activity_import_id: i32,
    pub user_id: i32,
    pub status: String,
    pub message: String,
    pub task_id: String,
    pub task_status: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CleanupUserDuplicateActivitiesRequest {
    pub user_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CleanupUserDuplicateActivitiesResponse {
    pub user_id: i32,
    pub status: String,
    pub message: String,
    pub duplicate_group_count: i32,
    pub deleted_activity_count: i32,
    pub retained_activity_count: i32,
}

#[utoipa::path(
    post,
    path = "/admin/analytics/backfill",
    operation_id = "admin_backfill_analytics",
    responses(
        (status = 202, description = "Enqueued analytics backfill tasks", body = AnalyticsBackfillResponse),
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
        state.tasks.rebuild_fitness_freshness(*user_id).await;
    }

    let segment_task_count = enqueue_segment_backfill_tasks(&state, &segment_ids).await;
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
    request_body = ReprocessUserActivityImportsRequest,
    responses(
        (status = 202, description = "Queued XC training backfill for one user", body = ReprocessUserActivityImportsResponse),
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
    Json(request): Json<ReprocessUserActivityImportsRequest>,
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
    request_body = RegenerateUserSegmentsRequest,
    responses(
        (status = 202, description = "Queued segment regeneration for one user", body = RegenerateUserSegmentsResponse),
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
    request_body = RegenerateSegmentEffortsRequest,
    responses(
        (status = 202, description = "Queued effort regeneration for one segment", body = RegenerateSegmentEffortsResponse),
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
    request_body = ReprocessUserActivityImportsRequest,
    responses(
        (status = 202, description = "Queued stored-file activity reprocessing for one user", body = ReprocessUserActivityImportsResponse),
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

    if let Err(message) = state
        .tasks
        .reprocess_user_activity_imports(request.user_id)
        .await
    {
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
    request_body = ReprocessActivityImportRequest,
    responses(
        (status = 202, description = "Queued stored-file activity reprocessing for one activity", body = ReprocessActivityImportResponse),
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
    request_body = CleanupUserDuplicateActivitiesRequest,
    responses(
        (status = 200, description = "Removed duplicate activities for one user", body = CleanupUserDuplicateActivitiesResponse),
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
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(response), Ok(())) => Ok(response),
    }
}

#[utoipa::path(
    post,
    path = "/admin/activity-imports/archive",
    operation_id = "admin_import_activity_archive",
    request_body = ArchiveImportRequest,
    responses(
        (status = 200, description = "Imported activities from an archive on the server", body = ArchiveImportResponse),
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
        &state.uploads_dir,
        &user_storage_key,
        admin.user.id,
        "archive_import",
        archive_path.display().to_string(),
        &archive_path,
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
        (Err(error), _) => Err(error),
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

async fn enqueue_segment_backfill_tasks(state: &Arc<AppStorage>, segment_ids: &[i32]) -> i32 {
    let mut task_count = 0;

    for chunk in segment_ids.chunks(SEGMENT_BACKFILL_CHUNK_SIZE) {
        state.tasks.rebuild_segment_analytics(chunk.to_vec()).await;
        task_count += 1;
    }

    task_count
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, Schema, Set};

    async fn test_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory db");

        let schema = Schema::new(db.get_database_backend());
        db.execute(&schema.create_table_from_entity(activities::Entity))
            .await
            .expect("create activities table");
        db.execute(&schema.create_table_from_entity(activity_imports::Entity))
            .await
            .expect("create activity imports table");
        db.execute(&schema.create_table_from_entity(segments::Entity))
            .await
            .expect("create segments table");
        db.execute(&schema.create_table_from_entity(strava_connections::Entity))
            .await
            .expect("create strava connections table");

        db
    }

    #[tokio::test]
    async fn empty_segment_backfill_enqueues_no_tasks() {
        let state = Arc::new(AppStorage {
            db: sea_orm::Database::connect("sqlite::memory:")
                .await
                .expect("in-memory db"),
            tasks: crate::tasks::TaskQueue::new(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .expect("in-memory queue db"),
            ),
            feature_flags: kaleido::glass::feature_flags::FeatureFlagService::new(),
            auth_service: crate::tasks::create_auth_service(
                sea_orm::Database::connect("sqlite::memory:")
                    .await
                    .expect("in-memory auth db"),
                crate::tasks::TaskQueue::new(
                    sea_orm::Database::connect("sqlite::memory:")
                        .await
                        .expect("in-memory auth queue db"),
                ),
            ),
            uploads_dir: "/tmp".to_string(),
        });

        assert_eq!(enqueue_segment_backfill_tasks(&state, &[]).await, 0);
    }

    #[tokio::test]
    async fn bike_metrics_returns_expected_named_stats() {
        let db = test_db().await;
        let now = Utc::now();

        let activity = activities::ActiveModel {
            user_id: Set(1),
            activity_import_id: Set(None),
            title: Set("Lunch Ride".to_string()),
            sport: Set("ride".to_string()),
            source: Set("manual_upload".to_string()),
            source_correlation_id: Set(None),
            original_filename: Set(None),
            format: Set(Some("gpx".to_string())),
            activity_type: Set(crate::activity_type::ActivityType::Training
                .as_str()
                .to_string()),
            started_at: Set(now - Duration::hours(2)),
            ended_at: Set(Some(now - Duration::hours(1))),
            distance_meters: Set(Some(25_000.0)),
            moving_time_seconds: Set(Some(3600)),
            total_time_seconds: Set(Some(3900)),
            elevation_gain_meters: Set(Some(400.0)),
            elevation_loss_meters: Set(Some(400.0)),
            average_speed_mps: Set(Some(7.0)),
            max_speed_mps: Set(Some(12.0)),
            average_heart_rate_bpm: Set(Some(140)),
            max_heart_rate_bpm: Set(Some(170)),
            average_cadence_rpm: Set(Some(85)),
            max_cadence_rpm: Set(Some(102)),
            calories: Set(Some(700)),
            estimated_ftp_watts: Set(None),
            heart_rate_zones_json: Set(None),
            derived_data_json: Set(None),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert activity");

        activity_imports::ActiveModel {
            user_id: Set(1),
            source: Set("manual_upload".to_string()),
            format: Set("gpx".to_string()),
            status: Set("processed".to_string()),
            activity_id: Set(Some(activity.id)),
            processing_stage: Set("complete".to_string()),
            processing_error: Set(None),
            processing_attempts: Set(0),
            processed_at: Set(Some(now)),
            last_processing_event_at: Set(Some(now)),
            original_filename: Set("lunch-ride.gpx".to_string()),
            storage_path: Set("/tmp/lunch-ride.gpx".to_string()),
            size_bytes: Set(1_024),
            mime_type: Set(Some("application/gpx+xml".to_string())),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert activity import");

        segments::ActiveModel {
            user_id: Set(1),
            title: Set("North Climb".to_string()),
            source: Set("manual_segment_import".to_string()),
            mode: Set("xc".to_string()),
            starred: Set(false),
            original_filename: Set(Some("north-climb.gpx".to_string())),
            format: Set(Some("gpx".to_string())),
            distance_meters: Set(Some(1800.0)),
            route_data_json: Set(None),
            last_activity_change_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert segment");

        strava_connections::ActiveModel {
            user_id: Set(1),
            athlete_id: Set(35_999_641),
            athlete_username: Set(Some("ericbutera".to_string())),
            athlete_first_name: Set(Some("Eric".to_string())),
            athlete_last_name: Set(Some("Butera".to_string())),
            athlete_profile_medium_url: Set(None),
            scopes: Set("activity:read".to_string()),
            access_token: Set("access-token".to_string()),
            refresh_token: Set("refresh-token".to_string()),
            expires_at: Set(now + Duration::hours(1)),
            last_synced_activity_started_at: Set(None),
            last_sync_status: Set("never".to_string()),
            last_sync_message: Set(None),
            last_sync_started_at: Set(None),
            last_sync_finished_at: Set(None),
            last_sync_imported_count: Set(0),
            last_sync_duplicate_count: Set(0),
            last_sync_failed_count: Set(0),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert strava connection");

        let stats = bike_metrics(&db).await;
        let values_by_key = stats
            .into_iter()
            .map(|stat| (stat.key, stat.value))
            .collect::<std::collections::HashMap<_, _>>();

        assert_eq!(values_by_key.get("total_activities"), Some(&1));
        assert_eq!(values_by_key.get("activities_added_last_30d"), Some(&1));
        assert_eq!(values_by_key.get("total_activity_imports"), Some(&1));
        assert_eq!(values_by_key.get("activity_imports_last_30d"), Some(&1));
        assert_eq!(values_by_key.get("total_segments"), Some(&1));
        assert_eq!(values_by_key.get("segments_added_last_30d"), Some(&1));
        assert_eq!(values_by_key.get("total_strava_connections"), Some(&1));
    }
}
