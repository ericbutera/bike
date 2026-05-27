use crate::activity_import_lock::{
    acquire_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING, ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
    ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP,
    ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION, ACTIVITY_IMPORT_LOCK_STAGE_QUEUED,
    ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_lifecycle::cleanup_duplicate_activities_for_user;
use crate::analytics::mark_user_activity_changes;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::archive_import::{import_activity_archive_from_path, resolve_local_archive_import_path};
use crate::entities::{activities, activity_imports, segments, strava_connections};
use crate::storage::AppStorage;
use crate::xc_goal_backfill::queue_user_xc_goal_backfill;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use kaleido::auth::entities::users;
use kaleido::auth::AdminUserContext;
use kaleido::glass::aggregator::{Aggregator, NamedStat};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const SEGMENT_BACKFILL_CHUNK_SIZE: usize = 250;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/metrics/app", get(app_metrics))
        .route("/analytics/backfill", post(backfill_analytics))
        .route("/training/xc-backfill", post(backfill_user_xc_training))
        .route("/segments/regenerate", post(regenerate_user_segments))
        .route(
            "/activity-imports/reprocess",
            post(reprocess_user_activity_imports),
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

        activities::ActiveModel {
            user_id: Set(1),
            activity_import_id: Set(None),
            title: Set("Lunch Ride".to_string()),
            sport: Set("ride".to_string()),
            source: Set("manual_upload".to_string()),
            source_correlation_id: Set(None),
            original_filename: Set(None),
            format: Set(Some("gpx".to_string())),
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
            status: Set("imported".to_string()),
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
