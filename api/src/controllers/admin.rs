use crate::archive_import::{import_activity_archive_from_path, resolve_local_archive_import_path};
use crate::activity_import_lock::{
    acquire_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT, ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::analytics::mark_user_activity_changes;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::{activities, segments};
use crate::storage::AppStorage;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use chrono::Utc;
use kaleido::auth::entities::users;
use kaleido::auth::AdminUserContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const SEGMENT_BACKFILL_CHUNK_SIZE: usize = 250;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/analytics/backfill", post(backfill_analytics))
        .route("/segments/regenerate", post(regenerate_user_segments))
        .route("/activity-imports/reprocess", post(reprocess_user_activity_imports))
        .route("/activity-imports/archive", post(import_activity_archive))
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
    let archive_path = resolve_local_archive_import_path(&state.uploads_dir, &request.archive_path)?;
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
}
