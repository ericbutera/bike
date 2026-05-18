use crate::activity_import_lock::{
    acquire_user_activity_import_lock, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, persist_activity_upload, validate_activity_format,
    ActivityUploadDeduplication, ActivityUploadPayload, PersistActivityUploadOutcome,
};
use crate::activity_location::location_from_derived_json;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::archive_import::{
    decode_error_samples, enqueue_activity_archive_import_job, normalize_archive_url,
};
use crate::config::Config;
use crate::entities::{activities, activity_archive_import_jobs, activity_imports};
use crate::storage::AppStorage;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::UserContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityImportResponse {
    pub id: i32,
    pub activity_id: Option<i32>,
    pub original_filename: String,
    pub format: String,
    pub status: String,
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    pub created_at: DateTime<Utc>,
    pub activity_started_at: Option<DateTime<Utc>>,
    pub activity_duration_seconds: Option<i32>,
    pub activity_location: Option<String>,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct ArchiveUrlImportRequest {
    pub archive_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityArchiveImportJobResponse {
    pub id: i32,
    pub archive_url: String,
    pub resolved_url: Option<String>,
    pub status: String,
    pub failure_message: Option<String>,
    pub total_entries: i32,
    pub supported_entry_count: i32,
    pub imported_count: i32,
    pub duplicate_count: i32,
    pub skipped_unsupported_count: i32,
    pub failed_count: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub error_samples: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

impl ActivityImportResponse {
    fn from_model(model: activity_imports::Model, activity: Option<&activities::Model>) -> Self {
        Self {
            id: model.id,
            activity_id: activity.map(|value| value.id),
            original_filename: model.original_filename,
            format: model.format,
            status: model.status,
            size_bytes: model.size_bytes,
            mime_type: model.mime_type,
            created_at: model.created_at,
            activity_started_at: activity.map(|value| value.started_at),
            activity_duration_seconds: activity
                .and_then(|value| value.moving_time_seconds.or(value.total_time_seconds)),
            activity_location: activity
                .and_then(|value| location_from_derived_json(value.derived_data_json.as_ref())),
        }
    }
}

impl ActivityArchiveImportJobResponse {
    fn from_model(model: activity_archive_import_jobs::Model) -> Self {
        Self {
            id: model.id,
            archive_url: model.archive_url,
            resolved_url: model.resolved_url,
            status: model.status,
            failure_message: model.failure_message,
            total_entries: model.total_entries,
            supported_entry_count: model.supported_entry_count,
            imported_count: model.imported_count,
            duplicate_count: model.duplicate_count,
            skipped_unsupported_count: model.skipped_unsupported_count,
            failed_count: model.failed_count,
            error_samples: decode_error_samples(model.error_samples_json.as_deref()),
            created_at: model.created_at,
            started_at: model.started_at,
            finished_at: model.finished_at,
            updated_at: model.updated_at,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/activity-imports",
    responses(
        (status = 200, description = "Recent activity imports for the authenticated user", body = [ActivityImportResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activity-imports",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_activity_imports(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<ActivityImportResponse>>, AppError> {
    let imports = activity_imports::Entity::find()
        .filter(activity_imports::Column::UserId.eq(user.id))
        .order_by_desc(activity_imports::Column::CreatedAt)
        .limit(10)
        .all(&state.db)
        .await?;

    let import_ids = imports.iter().map(|model| model.id).collect::<Vec<_>>();
    let activities_by_import_id = if import_ids.is_empty() {
        HashMap::new()
    } else {
        activities::Entity::find()
            .filter(activities::Column::UserId.eq(user.id))
            .filter(
                activities::Column::ActivityImportId
                    .is_in(import_ids.iter().copied().map(Some).collect::<Vec<_>>()),
            )
            .all(&state.db)
            .await?
            .into_iter()
            .filter_map(|activity| {
                activity
                    .activity_import_id
                    .map(|import_id| (import_id, activity))
            })
            .collect::<HashMap<_, _>>()
    };

    Ok(Json(
        imports
            .into_iter()
            .map(|model| {
                let activity = activities_by_import_id.get(&model.id);
                ActivityImportResponse::from_model(model, activity)
            })
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/activity-imports/archive-jobs",
    responses(
        (status = 200, description = "Recent archive import jobs for the authenticated user", body = [ActivityArchiveImportJobResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activity-imports",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_activity_archive_import_jobs(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<ActivityArchiveImportJobResponse>>, AppError> {
    let jobs = activity_archive_import_jobs::Entity::find()
        .filter(activity_archive_import_jobs::Column::UserId.eq(user.id))
        .order_by_desc(activity_archive_import_jobs::Column::CreatedAt)
        .limit(10)
        .all(&state.db)
        .await?;

    Ok(Json(
        jobs.into_iter()
            .map(ActivityArchiveImportJobResponse::from_model)
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/activity-imports/archive-jobs/{id}",
    params(("id" = i32, Path, description = "Archive import job id")),
    responses(
        (status = 200, description = "Archive import job status", body = ActivityArchiveImportJobResponse),
        (status = 401, description = "Not authenticated"),
        (status = 404, description = "Archive import job not found", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activity-imports",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_activity_archive_import_job(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Path(id): Path<i32>,
) -> Result<Json<ActivityArchiveImportJobResponse>, AppError> {
    let job = activity_archive_import_jobs::Entity::find_by_id(id)
        .filter(activity_archive_import_jobs::Column::UserId.eq(user.id))
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("Archive import job not found"))?;

    Ok(Json(ActivityArchiveImportJobResponse::from_model(job)))
}

#[utoipa::path(
    post,
    path = "/api/activity-imports",
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Activity was already imported and the existing record was returned", body = ActivityImportResponse),
        (status = 201, description = "Activity import uploaded", body = ActivityImportResponse),
        (status = 400, description = "Invalid upload", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 413, description = "Payload too large", body = ApiErrorResponse),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activity-imports",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn upload_activity_import(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    multipart: Multipart,
) -> Result<(StatusCode, Json<ActivityImportResponse>), AppError> {
    let upload = read_uploaded_activity_file(multipart).await?;
    let upload_for_duplicate = upload.clone();
    let user_storage_key = user.pid.to_string();
    acquire_user_activity_import_lock(
        &state.db,
        user.id,
        ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = async {
        let outcome = persist_activity_upload(
            &state.db,
            &state.uploads_dir,
            &user_storage_key,
            user.id,
            upload,
            "manual_upload",
            ActivityUploadDeduplication::Enabled,
            None,
        )
        .await?;

        match outcome {
            PersistActivityUploadOutcome::Imported(persisted) => {
                finalize_activity_import_batch(
                    &state.db,
                    &state.tasks,
                    user.id,
                    persisted.affected_segment_ids.clone(),
                    Utc::now(),
                )
                .await?;

                Ok((
                    StatusCode::CREATED,
                    Json(ActivityImportResponse::from_model(
                        persisted.import,
                        Some(&persisted.activity),
                    )),
                ))
            }
            PersistActivityUploadOutcome::Duplicate(duplicate) => Ok((
                StatusCode::OK,
                Json(build_duplicate_activity_import_response(
                    &upload_for_duplicate,
                    "manual_upload",
                    duplicate,
                )),
            )),
        }
    }
    .await;

    let release_result = release_user_activity_import_lock(
        &state.db,
        user.id,
        ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD,
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
    path = "/api/activity-imports/archive-url",
    request_body = ArchiveUrlImportRequest,
    responses(
        (status = 202, description = "Archive import job queued", body = ActivityArchiveImportJobResponse),
        (status = 400, description = "Invalid archive URL", body = ApiErrorResponse),
        (status = 409, description = "Another activity import is already running or queued", body = ApiErrorResponse),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "activity-imports",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn import_activity_archive_from_url(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Json(request): Json<ArchiveUrlImportRequest>,
) -> Result<(StatusCode, Json<ActivityArchiveImportJobResponse>), AppError> {
    let archive_url = normalize_archive_url(&request.archive_url)?;
    let user_storage_key = user.pid.to_string();
    let job = enqueue_activity_archive_import_job(
        &state.db,
        &state.tasks,
        user.id,
        &user_storage_key,
        archive_url,
    )
    .await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(ActivityArchiveImportJobResponse::from_model(job)),
    ))
}

async fn read_uploaded_activity_file(
    mut multipart: Multipart,
) -> Result<ActivityUploadPayload, AppError> {
    let max_upload_bytes = Config::get().max_upload_bytes;

    while let Some(mut field) = multipart.next_field().await.map_err(|err| {
        map_multipart_error(
            &err.to_string(),
            max_upload_bytes,
            "Malformed multipart payload",
        )
    })? {
        if field.name() != Some("file") && field.file_name().is_none() {
            continue;
        }

        let original_filename = field
            .file_name()
            .map(|value| value.to_string())
            .ok_or_else(|| {
                AppError::validation_field("file", "Uploaded file is missing a filename")
            })?;
        let format = validate_activity_format(&original_filename)?;
        let mime_type = field.content_type().map(|value| value.to_string());
        let mut bytes = Vec::new();
        let mut total_bytes = 0usize;

        while let Some(chunk) = field.chunk().await.map_err(|err| {
            map_multipart_error(
                &err.to_string(),
                max_upload_bytes,
                "Failed to read upload field",
            )
        })? {
            total_bytes += chunk.len();
            if total_bytes > max_upload_bytes {
                return Err(AppError::payload_too_large(
                    "file",
                    format!("File exceeds the {} byte upload limit", max_upload_bytes),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        if bytes.is_empty() {
            return Err(AppError::validation_field("file", "Uploaded file is empty"));
        }

        return Ok(ActivityUploadPayload {
            original_filename,
            format,
            mime_type,
            source_correlation_id: None,
            bytes,
        });
    }

    Err(AppError::validation_field(
        "file",
        "A .fit, .tcx, or .gpx file is required",
    ))
}

fn map_multipart_error(
    error_text: &str,
    max_upload_bytes: usize,
    default_message: &str,
) -> AppError {
    let normalized = error_text.to_ascii_lowercase();
    let is_too_large = normalized.contains("body too large")
        || normalized.contains("field too large")
        || normalized.contains("payload too large")
        || normalized.contains("failed to read stream")
        || normalized.contains("request body is malformed")
        || normalized.contains("length limit")
        || normalized.contains("size limit");

    if is_too_large {
        return AppError::payload_too_large(
            "file",
            format!("File exceeds the {} byte upload limit", max_upload_bytes),
        );
    }

    AppError::bad_request(format!("{default_message}: {error_text}"))
}

fn build_duplicate_activity_import_response(
    upload: &ActivityUploadPayload,
    source: &str,
    duplicate: crate::activity_import_pipeline::DeduplicatedActivityImport,
) -> ActivityImportResponse {
    let import_model = duplicate
        .existing_import
        .unwrap_or(activity_imports::Model {
            id: 0,
            user_id: duplicate.activity.user_id,
            source: source.to_string(),
            format: upload.format.clone(),
            status: "duplicate".to_string(),
            original_filename: upload.original_filename.clone(),
            storage_path: String::new(),
            size_bytes: upload.bytes.len() as i64,
            mime_type: upload.mime_type.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        });
    let mut response = ActivityImportResponse::from_model(import_model, Some(&duplicate.activity));
    response.status = "duplicate".to_string();
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity_details::serialize_derived_activity_data;

    #[test]
    fn validate_activity_format_accepts_supported_extensions() {
        assert_eq!(validate_activity_format("ride.fit").unwrap(), "fit");
        assert_eq!(validate_activity_format("ride.tcx").unwrap(), "tcx");
        assert_eq!(validate_activity_format("ride.gpx").unwrap(), "gpx");
    }

    #[test]
    fn validate_activity_format_normalizes_uppercase_extensions() {
        assert_eq!(validate_activity_format("RIDE.GPX").unwrap(), "gpx");
        assert_eq!(validate_activity_format("RIDE.FIT").unwrap(), "fit");
    }

    #[test]
    fn validate_activity_format_rejects_missing_extension() {
        let error = validate_activity_format("ride").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "Uploaded file must include a .fit, .tcx, or .gpx extension"
        );
        assert_eq!(
            error.errors.unwrap().get("file").unwrap(),
            &vec!["Uploaded file must include a .fit, .tcx, or .gpx extension".to_string()]
        );
    }

    #[test]
    fn validate_activity_format_rejects_unsupported_extensions() {
        let error = validate_activity_format("ride.csv").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "Only .fit, .tcx, and .gpx uploads are supported"
        );
        assert_eq!(
            error.errors.unwrap().get("file").unwrap(),
            &vec!["Only .fit, .tcx, and .gpx uploads are supported".to_string()]
        );
    }

    #[test]
    fn map_multipart_error_classifies_oversized_payloads() {
        let error = map_multipart_error(
            "Error parsing `multipart/form-data` request: Request body is malformed",
            1024,
            "Failed to read upload field",
        );

        assert_eq!(error.status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(error.message, "File exceeds the 1024 byte upload limit");
        assert_eq!(
            error.errors.unwrap().get("file").unwrap(),
            &vec!["File exceeds the 1024 byte upload limit".to_string()]
        );
    }

    #[test]
    fn map_multipart_error_preserves_non_size_parse_failures() {
        let error = map_multipart_error("missing boundary", 1024, "Malformed multipart payload");

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "Malformed multipart payload: missing boundary"
        );
        assert!(error.errors.is_none());
    }

    #[test]
    fn activity_archive_import_job_response_maps_error_samples() {
        let now = Utc::now();
        let response =
            ActivityArchiveImportJobResponse::from_model(activity_archive_import_jobs::Model {
                id: 5,
                user_id: 12,
                user_storage_key: "user-12".to_string(),
                archive_url: "https://example.com/export.zip".to_string(),
                resolved_url: Some("https://cdn.example.com/export.zip".to_string()),
                status: "succeeded".to_string(),
                failure_message: None,
                error_samples_json: Some(
                    serde_json::to_string(&vec!["bad.fit: parse failed".to_string()])
                        .expect("serialize error samples"),
                ),
                total_entries: 10,
                supported_entry_count: 8,
                imported_count: 7,
                duplicate_count: 1,
                skipped_unsupported_count: 2,
                failed_count: 0,
                created_at: now,
                started_at: Some(now),
                finished_at: Some(now),
                updated_at: now,
            });

        assert_eq!(response.id, 5);
        assert_eq!(response.archive_url, "https://example.com/export.zip");
        assert_eq!(response.status, "succeeded");
        assert_eq!(response.imported_count, 7);
        assert_eq!(response.error_samples, vec!["bad.fit: parse failed"]);
    }

    #[test]
    fn activity_import_response_maps_model_fields() {
        let now = Utc::now();
        let activity = activities::Model {
            id: 21,
            user_id: 12,
            activity_import_id: Some(7),
            title: "Ride".to_string(),
            sport: "ride".to_string(),
            source: "manual_upload".to_string(),
            source_correlation_id: None,
            original_filename: Some("ride.gpx".to_string()),
            format: Some("gpx".to_string()),
            started_at: now,
            ended_at: Some(now),
            distance_meters: Some(25000.0),
            moving_time_seconds: Some(3600),
            total_time_seconds: Some(3650),
            elevation_gain_meters: Some(320.0),
            elevation_loss_meters: Some(315.0),
            average_speed_mps: Some(7.2),
            max_speed_mps: Some(12.4),
            average_heart_rate_bpm: Some(140),
            max_heart_rate_bpm: Some(172),
            average_cadence_rpm: Some(86),
            max_cadence_rpm: Some(102),
            calories: Some(650),
            estimated_ftp_watts: None,
            heart_rate_zones_json: None,
            derived_data_json: Some(
                serialize_derived_activity_data(&crate::activity_details::ActivityDerivedData {
                    laps: Vec::new(),
                    chart_points: Vec::new(),
                    route_points: vec![crate::activity_details::ActivityRoutePoint {
                        elapsed_seconds: 0,
                        latitude: 45.523,
                        longitude: -122.676,
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
        };
        let response = ActivityImportResponse::from_model(
            activity_imports::Model {
                id: 7,
                user_id: 12,
                source: "manual_upload".to_string(),
                format: "gpx".to_string(),
                status: "uploaded".to_string(),
                original_filename: "ride.gpx".to_string(),
                storage_path: "activity-imports/user/ride.gpx".to_string(),
                size_bytes: 8192,
                mime_type: Some("application/gpx+xml".to_string()),
                created_at: now,
                updated_at: now,
            },
            Some(&activity),
        );

        assert_eq!(response.id, 7);
        assert_eq!(response.activity_id, Some(21));
        assert_eq!(response.original_filename, "ride.gpx");
        assert_eq!(response.format, "gpx");
        assert_eq!(response.status, "uploaded");
        assert_eq!(response.size_bytes, 8192);
        assert_eq!(response.mime_type.as_deref(), Some("application/gpx+xml"));
        assert_eq!(response.created_at, now);
        assert_eq!(response.activity_started_at, Some(now));
        assert_eq!(response.activity_duration_seconds, Some(3600));
        assert!(response.activity_location.is_some());
    }
}
