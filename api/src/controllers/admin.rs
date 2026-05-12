use crate::activity_import_lock::{
    acquire_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT, ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, persist_activity_upload, ActivityUploadPayload,
    PersistActivityUploadOutcome,
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
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use utoipa::ToSchema;
use zip::ZipArchive;

const SEGMENT_BACKFILL_CHUNK_SIZE: usize = 250;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .route("/analytics/backfill", post(backfill_analytics))
        .route("/segments/regenerate", post(regenerate_user_segments))
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
    let archive_path = resolve_archive_import_path(&state.uploads_dir, &request.archive_path)?;
    let scan = scan_archive_entries(&archive_path)?;
    let training_profile =
        crate::training_profile::load_training_profile(&state.db, admin.user.id).await?;
    let mut imported_count = 0i32;
    let mut duplicate_count = 0i32;
    let mut affected_segment_ids = Vec::new();
    let mut error_samples = Vec::new();
    let user_storage_key = admin.user.pid.to_string();
    acquire_user_activity_import_lock(
        &state.db,
        admin.user.id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = async {
        for indexed_entry in &scan.supported_entries {
            let bytes = match read_archive_entry_bytes(&archive_path, indexed_entry.index) {
                Ok(value) => value,
                Err(error) => {
                    error_samples.push(format!(
                        "{}: failed to read archive entry: {}",
                        indexed_entry.entry_name, error.message
                    ));
                    continue;
                }
            };

            let bytes = match maybe_decode_archive_entry(&indexed_entry.activity_entry, bytes) {
                Ok(value) => value,
                Err(message) => {
                    error_samples.push(format!("{}: {}", indexed_entry.entry_name, message));
                    continue;
                }
            };

            let upload = ActivityUploadPayload {
                original_filename: indexed_entry.activity_entry.original_filename.clone(),
                format: indexed_entry.activity_entry.format.clone(),
                mime_type: None,
                bytes,
            };

            match persist_activity_upload(
                &state.db,
                &state.uploads_dir,
                &user_storage_key,
                admin.user.id,
                upload,
                "archive_import",
                Some(&training_profile),
            )
            .await
            {
                Ok(PersistActivityUploadOutcome::Imported(persisted)) => {
                    imported_count += 1;
                    affected_segment_ids.extend(persisted.affected_segment_ids);
                }
                Ok(PersistActivityUploadOutcome::Duplicate(_)) => {
                    duplicate_count += 1;
                }
                Err(error) => {
                    error_samples.push(format!("{}: {}", indexed_entry.entry_name, error.message));
                }
            }
        }

        if scan.supported_entry_count == 0 {
            return Err(AppError::validation_field(
                "archive_path",
                "Archive did not contain any supported .fit, .tcx, or .gpx files",
            ));
        }

        if imported_count > 0 {
            finalize_activity_import_batch(
                &state.db,
                &state.tasks,
                admin.user.id,
                affected_segment_ids,
                Utc::now(),
            )
            .await?;
        }

        let failed_count = error_samples.len() as i32;
        let error_samples = error_samples.into_iter().take(10).collect::<Vec<_>>();

        Ok((
            StatusCode::OK,
            Json(ArchiveImportResponse {
                archive_path: archive_path.display().to_string(),
                total_entries: scan.total_entries,
                supported_entry_count: scan.supported_entry_count,
                imported_count,
                duplicate_count,
                skipped_unsupported_count: scan.skipped_unsupported_count,
                failed_count,
                error_samples,
            }),
        ))
    }
    .await;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchiveActivityEntry {
    original_filename: String,
    format: String,
    gzip_wrapped: bool,
}

#[derive(Debug, Clone)]
struct IndexedArchiveActivityEntry {
    index: usize,
    entry_name: String,
    activity_entry: ArchiveActivityEntry,
}

#[derive(Debug, Clone)]
struct ArchiveScanResult {
    total_entries: i32,
    supported_entry_count: i32,
    skipped_unsupported_count: i32,
    supported_entries: Vec<IndexedArchiveActivityEntry>,
}

fn resolve_archive_import_path(uploads_dir: &str, archive_path: &str) -> Result<PathBuf, AppError> {
    let trimmed = archive_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation_field(
            "archive_path",
            "Archive path is required",
        ));
    }

    let requested = PathBuf::from(trimmed);
    let resolved = if requested.is_absolute() {
        requested
    } else {
        Path::new(uploads_dir).join(requested)
    };

    let extension = resolved
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());

    if extension.as_deref() != Some("zip") {
        return Err(AppError::validation_field(
            "archive_path",
            "Archive path must point to a .zip file",
        ));
    }

    if !resolved.exists() {
        return Err(AppError::not_found(format!(
            "Archive not found: {}",
            resolved.display()
        )));
    }

    if !resolved.is_file() {
        return Err(AppError::validation_field(
            "archive_path",
            "Archive path must point to a file",
        ));
    }

    Ok(resolved)
}

fn resolve_archive_activity_entry(name: &str) -> Option<ArchiveActivityEntry> {
    let file_name = Path::new(name).file_name()?.to_str()?.trim();
    if file_name.is_empty() {
        return None;
    }

    let lower = file_name.to_ascii_lowercase();
    for (suffix, format, gzip_wrapped) in [
        (".fit.gz", "fit", true),
        (".tcx.gz", "tcx", true),
        (".gpx.gz", "gpx", true),
        (".fit", "fit", false),
        (".tcx", "tcx", false),
        (".gpx", "gpx", false),
    ] {
        if lower.ends_with(suffix) {
            let original_filename = if gzip_wrapped {
                file_name[..file_name.len() - 3].to_string()
            } else {
                file_name.to_string()
            };

            return Some(ArchiveActivityEntry {
                original_filename,
                format: format.to_string(),
                gzip_wrapped,
            });
        }
    }

    None
}

fn scan_archive_entries(archive_path: &Path) -> Result<ArchiveScanResult, AppError> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::bad_request(format!("Failed to open zip archive: {error}")))?;
    let mut total_entries = 0i32;
    let mut skipped_unsupported_count = 0i32;
    let mut supported_entries = Vec::new();

    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(|error| {
            AppError::bad_request(format!("Failed to read zip archive entry: {error}"))
        })?;

        if entry.is_dir() {
            continue;
        }

        total_entries += 1;
        let entry_name = entry.name().to_string();

        if let Some(activity_entry) = resolve_archive_activity_entry(&entry_name) {
            supported_entries.push(IndexedArchiveActivityEntry {
                index,
                entry_name,
                activity_entry,
            });
        } else {
            skipped_unsupported_count += 1;
        }
    }

    Ok(ArchiveScanResult {
        total_entries,
        supported_entry_count: supported_entries.len() as i32,
        skipped_unsupported_count,
        supported_entries,
    })
}

fn read_archive_entry_bytes(archive_path: &Path, index: usize) -> Result<Vec<u8>, AppError> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| AppError::bad_request(format!("Failed to open zip archive: {error}")))?;
    let mut entry = archive.by_index(index).map_err(|error| {
        AppError::bad_request(format!("Failed to read zip archive entry: {error}"))
    })?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn maybe_decode_archive_entry(
    entry: &ArchiveActivityEntry,
    bytes: Vec<u8>,
) -> Result<Vec<u8>, String> {
    if !entry.gzip_wrapped {
        return Ok(bytes);
    }

    let mut decoder = flate2::read::GzDecoder::new(bytes.as_slice());
    let mut decoded = Vec::new();
    decoder
        .read_to_end(&mut decoded)
        .map_err(|error| format!("failed to decode gzip-compressed activity: {}", error))?;

    Ok(decoded)
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

    #[test]
    fn resolves_supported_archive_entries() {
        assert_eq!(
            resolve_archive_activity_entry("activities/ride.fit.gz"),
            Some(ArchiveActivityEntry {
                original_filename: "ride.fit".to_string(),
                format: "fit".to_string(),
                gzip_wrapped: true,
            })
        );
        assert_eq!(
            resolve_archive_activity_entry("garmin/ride.TCX"),
            Some(ArchiveActivityEntry {
                original_filename: "ride.TCX".to_string(),
                format: "tcx".to_string(),
                gzip_wrapped: false,
            })
        );
    }

    #[test]
    fn ignores_unsupported_archive_entries() {
        assert_eq!(resolve_archive_activity_entry("activities.csv"), None);
        assert_eq!(resolve_archive_activity_entry("photos/ride.jpg"), None);
    }
}
