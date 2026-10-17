use crate::activity_location::location_from_derived_json;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::storage::AppStorage;
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use bike_core::activity_import_pipeline::{
    find_stored_activity_import, mark_activity_import_failed, store_activity_upload_import, validate_activity_format,
    ActivityUploadPayload,
};
use bike_core::archive_import::{decode_error_samples, enqueue_activity_archive_import_job, normalize_archive_url};
use bike_core::auth::UserContext;
use bike_core::config::Config;
use bike_core::entities::{activities, activity_archive_import_jobs, activity_imports};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(ToSchema)]
struct ActivityImportUploadForm {
    #[schema(value_type = String, format = Binary)]
    #[expect(dead_code, reason = "documents the multipart field consumed by Axum")]
    file: Vec<u8>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 17,"import_version": 1,"original_filename": "saturday-hills.fit","format": "fit","status": "processed","processing_stage": "complete","size_bytes": 184320,"created_at": "2026-09-26T14:20:00Z","activity_id": 42,"activity_started_at": "2026-09-26T13:00:00Z","activity_location": "Detroit, Michigan","activity_duration_seconds": 4320}))]
pub struct ActivityImportResponse {
    pub archive_job_id: Option<i32>,
    pub source: String,
    #[schema(example = 17)]
    pub id: i32,
    #[schema(example = 1)]
    pub import_version: i32,
    #[schema(example = 42)]
    pub activity_id: Option<i32>,
    #[schema(example = "saturday-hills.fit")]
    pub original_filename: String,
    #[schema(example = "fit")]
    pub format: String,
    #[schema(example = "processed")]
    pub status: String,
    #[schema(example = "complete")]
    pub processing_stage: String,
    pub processing_error: Option<String>,
    #[schema(example = 184320)]
    pub size_bytes: i64,
    pub mime_type: Option<String>,
    #[schema(example = "2026-09-26T14:20:00Z")]
    pub created_at: DateTime<Utc>,
    #[schema(example = "2026-09-26T13:00:00Z")]
    pub activity_started_at: Option<DateTime<Utc>>,
    #[schema(example = 4320)]
    pub activity_duration_seconds: Option<i32>,
    #[schema(example = "Detroit, Michigan")]
    pub activity_location: Option<String>,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
#[schema(example = json!({"archive_url": "https://example.com/exports/activities.zip"}))]
pub struct ArchiveUrlImportRequest {
    #[schema(example = "https://example.com/exports/activities.zip")]
    pub archive_url: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(example = json!({"id": 18,"archive_url": "https://example.com/exports/activities.zip","status": "queued","total_entries": 0,"supported_entry_count": 0,"imported_count": 0,"duplicate_count": 0,"skipped_unsupported_count": 0,"failed_count": 0,"created_at": "2026-09-27T12:00:00Z","updated_at": "2026-09-27T12:00:00Z","error_samples": []}))]
pub struct ActivityArchiveImportJobResponse {
    #[schema(example = 18)]
    pub id: i32,
    #[schema(example = "https://example.com/exports/activities.zip")]
    pub archive_url: String,
    pub resolved_url: Option<String>,
    #[schema(example = "queued")]
    pub status: String,
    pub failure_message: Option<String>,
    #[schema(example = 0)]
    pub total_entries: i32,
    #[schema(example = 0)]
    pub supported_entry_count: i32,
    #[schema(example = 0)]
    pub imported_count: i32,
    #[schema(example = 0)]
    pub duplicate_count: i32,
    #[schema(example = 0)]
    pub skipped_unsupported_count: i32,
    #[schema(example = 0)]
    pub failed_count: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schema(example = json!([]))]
    pub error_samples: Vec<String>,
    #[schema(example = "2026-09-27T12:00:00Z")]
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    #[schema(example = "2026-09-27T12:00:00Z")]
    pub updated_at: DateTime<Utc>,
}

impl ActivityImportResponse {
    pub(crate) fn from_model(model: activity_imports::Model, activity: Option<&activities::Model>) -> Self {
        Self {
            id: model.id,
            source: model.source,
            archive_job_id: model.archive_job_id,
            import_version: model.import_version,
            activity_id: activity.map(|value| value.id).or(model.activity_id),
            original_filename: model.original_filename,
            format: model.format,
            status: model.status,
            processing_stage: model.processing_stage,
            processing_error: model.processing_error,
            size_bytes: model.size_bytes,
            mime_type: model.mime_type,
            created_at: model.created_at,
            activity_started_at: activity.map(|value| value.started_at),
            activity_duration_seconds: activity
                .and_then(|value| value.moving_time_seconds.or(value.total_time_seconds)),
            activity_location: activity.and_then(|value| location_from_derived_json(value.derived_data_json.as_ref())),
        }
    }
}

impl ActivityArchiveImportJobResponse {
    pub(crate) fn from_model(model: activity_archive_import_jobs::Model) -> Self {
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
    path = "/activity-imports",
    responses(
        (status = 200, description = "Recent activity imports for the authenticated user", body = [ActivityImportResponse], example = json!([{"id": 17,"import_version": 1,"original_filename": "saturday-hills.fit","format": "fit","status": "processed","processing_stage": "complete","size_bytes": 184320,"created_at": "2026-09-26T14:20:00Z","activity_id": 42,"activity_started_at": "2026-09-26T13:00:00Z","activity_location": "Detroit, Michigan","activity_duration_seconds": 4320}])),
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
    let imports = activity_imports::Entity::recent_manual_uploads(&state.db, user.id).await?;
    let activities_by_import_id = activities::Model::linked_to_imports(&state.db, user.id, &imports).await?;

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
    path = "/activity-imports/archive-jobs",
    responses(
        (status = 200, description = "Recent archive import jobs for the authenticated user", body = [ActivityArchiveImportJobResponse], example = json!([{"id": 18,"archive_url": "https://example.com/exports/activities.zip","status": "queued","total_entries": 0,"supported_entry_count": 0,"imported_count": 0,"duplicate_count": 0,"skipped_unsupported_count": 0,"failed_count": 0,"created_at": "2026-09-27T12:00:00Z","updated_at": "2026-09-27T12:00:00Z","error_samples": []}])),
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
    let jobs = activity_archive_import_jobs::Entity::recent_for_user(&state.db, user.id).await?;

    Ok(Json(
        jobs.into_iter()
            .map(ActivityArchiveImportJobResponse::from_model)
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/activity-imports/archive-jobs/{id}",
    params(("id" = i32, Path, description = "Archive import job id", example = 17)),
    responses(
        (status = 200, description = "Archive import job status", body = ActivityArchiveImportJobResponse, example = json!({"id": 18,"archive_url": "https://example.com/exports/activities.zip","status": "queued","total_entries": 0,"supported_entry_count": 0,"imported_count": 0,"duplicate_count": 0,"skipped_unsupported_count": 0,"failed_count": 0,"created_at": "2026-09-27T12:00:00Z","updated_at": "2026-09-27T12:00:00Z","error_samples": []})),
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
    let job = activity_archive_import_jobs::Entity::find_owned(&state.db, user.id, id)
        .await?
        .ok_or_else(|| AppError::not_found("Archive import job not found"))?;

    Ok(Json(ActivityArchiveImportJobResponse::from_model(job)))
}

#[utoipa::path(
    post,
    path = "/activity-imports",
    request_body(content = inline(ActivityImportUploadForm), content_type = "multipart/form-data"),
    responses(
        (status = 200, description = "Activity was already imported and the existing record was returned", body = ActivityImportResponse, example = json!({"id": 17,"import_version": 1,"original_filename": "saturday-hills.fit","format": "fit","status": "processed","processing_stage": "complete","size_bytes": 184320,"created_at": "2026-09-26T14:20:00Z","activity_id": 42,"activity_started_at": "2026-09-26T13:00:00Z","activity_location": "Detroit, Michigan","activity_duration_seconds": 4320})),
        (status = 202, description = "Activity import queued for worker processing", body = ActivityImportResponse, example = json!({"id": 17,"import_version": 1,"original_filename": "saturday-hills.fit","format": "fit","status": "processing","processing_stage": "raw_stored","size_bytes": 184320,"created_at": "2026-09-26T14:20:00Z"})),
        (status = 400, description = "Invalid upload", body = ApiErrorResponse),
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
    let user_storage_key = user.pid.to_string();
    let import = match find_stored_activity_import(&state.db, user.id, &upload.bytes).await? {
        Some(existing) if existing.activity_id.is_some() => {
            return Ok((StatusCode::OK, Json(ActivityImportResponse::from_model(existing, None))));
        }
        Some(existing) => {
            existing
                .mark_processing(&state.db, "raw_stored", None, Utc::now())
                .await?
        }
        None => {
            store_activity_upload_import(
                &state.db,
                &state.uploads_dir,
                &user_storage_key,
                user.id,
                upload,
                "manual_upload",
            )
            .await?
        }
    };

    if let Err(failure) = state.tasks.process_activity_import(user.id, import.id).await {
        let error = AppError::internal(format!("Failed to queue activity import: {failure}"));
        let workflow_error = bike_core::workflow_error::WorkflowError::internal(error.message.clone());
        mark_activity_import_failed(&state.db, &import, &import.processing_stage, &workflow_error)
            .await
            .map_err(AppError::from)?;
        return Err(error);
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(ActivityImportResponse::from_model(import, None)),
    ))
}

#[utoipa::path(
    post,
    path = "/activity-imports/archive-url",
    request_body(content = ArchiveUrlImportRequest, example = json!({"archive_url": "https://example.com/exports/activities.zip"})),
    responses(
        (status = 202, description = "Archive import job queued", body = ActivityArchiveImportJobResponse, example = json!({"id": 18,"archive_url": "https://example.com/exports/activities.zip","status": "queued","total_entries": 0,"supported_entry_count": 0,"imported_count": 0,"duplicate_count": 0,"skipped_unsupported_count": 0,"failed_count": 0,"created_at": "2026-09-27T12:00:00Z","updated_at": "2026-09-27T12:00:00Z","error_samples": []})),
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
    let job =
        enqueue_activity_archive_import_job(&state.db, &state.tasks, user.id, &user_storage_key, archive_url).await?;

    Ok((
        StatusCode::ACCEPTED,
        Json(ActivityArchiveImportJobResponse::from_model(job)),
    ))
}

async fn read_uploaded_activity_file(mut multipart: Multipart) -> Result<ActivityUploadPayload, AppError> {
    let max_upload_bytes = Config::get().max_upload_bytes;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|err| map_multipart_error(&err.to_string(), max_upload_bytes, "Malformed multipart payload"))?
    {
        if field.name() != Some("file") && field.file_name().is_none() {
            continue;
        }

        let original_filename = field
            .file_name()
            .map(|value| value.to_string())
            .ok_or_else(|| AppError::validation_field("file", "Uploaded file is missing a filename"))?;
        let format = validate_activity_format(&original_filename)?;
        let mime_type = field.content_type().map(|value| value.to_string());
        let mut bytes = Vec::new();
        let mut total_bytes = 0usize;

        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|err| map_multipart_error(&err.to_string(), max_upload_bytes, "Failed to read upload field"))?
        {
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

fn map_multipart_error(error_text: &str, max_upload_bytes: usize, default_message: &str) -> AppError {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::activity_details::{serialize_derived_activity_data, ActivityDerivedData, ActivityRoutePoint};
    use chrono::Utc;

    #[tokio::test]
    async fn uploading_a_retained_unprocessed_file_retries_while_completed_imports_stay_duplicates() {
        use bike_core::test_support as fixtures;
        use std::sync::{Arc, Mutex};
        for retry in [false, true] {
            let import = activity_imports::Model {
                activity_id: (!retry).then_some(42),
                status: if retry { "failed" } else { "processed" }.into(),
                ..fixtures::reprocess_import()
            };
            let processing = activity_imports::Model {
                status: "processing".into(),
                ..import.clone()
            };
            let mut mock = fixtures::postgres_mock()
                .append_query_results([vec![fixtures::customer()]])
                .append_query_results([vec![fixtures::reprocess_artifact()]])
                .append_query_results([vec![import]]);
            if retry {
                mock = mock
                    .append_query_results([vec![processing.clone()], vec![processing]])
                    .append_query_results([vec![fixtures::reprocess_artifact()]]);
            }
            let requests = Arc::new(Mutex::new(fixtures::temporal_client::Requests::default()));
            let db = mock.into_connection();
            let mut state = AppStorage::for_test(db.clone());
            state.tasks = bike_core::jobs::JobQueue::with_client(
                db.clone(),
                fixtures::temporal_client::client(requests.clone()).await,
            );
            let result = crate::controllers::test_support::local_user_request(
                state,
                crate::controllers::test_support::activity_upload_request(),
                if retry { 202 } else { 200 },
            )
            .await;
            assert_eq!(result["id"], 18);
            assert_eq!(result["status"], if retry { "processing" } else { "processed" });
            let requests = requests.lock().unwrap();
            assert_eq!(requests.starts.len(), usize::from(retry));
            if retry {
                assert_eq!(
                    requests.starts[0].workflow_type.as_ref().unwrap().name,
                    "process_activity_import"
                );
            }
            let log = format!("{:?}", db.into_transaction_log());
            assert!(!log.contains("INSERT INTO"));
            assert!(!log.contains("background_tasks"));
        }
    }

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
        assert_eq!(error.message, "Only .fit, .tcx, and .gpx uploads are supported");
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
        assert_eq!(error.message, "Malformed multipart payload: missing boundary");
        assert!(error.errors.is_none());
    }

    #[test]
    fn activity_archive_import_job_response_maps_error_samples() {
        let now = bike_core::test_support::timestamp();
        let response = ActivityArchiveImportJobResponse::from_model(activity_archive_import_jobs::Model {
            id: 5,
            user_id: 12,
            user_storage_key: "user-12".to_string(),
            archive_url: "https://example.com/export.zip".to_string(),
            resolved_url: Some("https://cdn.example.com/export.zip".to_string()),
            status: "succeeded".to_string(),
            error_samples_json: Some(
                serde_json::to_string(&vec!["bad.fit: parse failed".to_string()]).expect("serialize error samples"),
            ),
            total_entries: 10,
            supported_entry_count: 8,
            imported_count: 7,
            duplicate_count: 1,
            skipped_unsupported_count: 2,
            created_at: now,
            started_at: Some(now),
            finished_at: Some(now),
            updated_at: now,
            ..bike_core::test_support::archive_import_job()
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
        let activity = activity_import_activity_fixture(now);
        let response = ActivityImportResponse::from_model(activity_import_fixture(now), Some(&activity));

        assert_eq!(response.id, 7);
        assert_eq!(
            response.import_version,
            bike_core::entities::activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT
        );
        assert_eq!(response.activity_id, Some(21));
        assert_eq!(response.original_filename, "ride.gpx");
        assert_eq!(response.format, "gpx");
        assert_eq!(response.status, "uploaded");
        assert_eq!(response.processing_stage, "complete");
        assert_eq!(response.processing_error, None);
        assert_eq!(response.size_bytes, 8192);
        assert_eq!(response.mime_type.as_deref(), Some("application/gpx+xml"));
        assert_eq!(response.created_at, now);
        assert_eq!(response.activity_started_at, Some(now));
        assert_eq!(response.activity_duration_seconds, Some(3600));
        assert!(response.activity_location.is_some());
    }

    fn activity_import_activity_fixture(now: DateTime<Utc>) -> activities::Model {
        activities::Model {
            id: 21,
            user_id: 12,
            activity_import_id: Some(7),
            title: "Ride".to_string(),
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
            derived_data_json: Some(activity_import_derived_data_json()),
            created_at: now,
            updated_at: now,
            ..bike_core::test_support::activity()
        }
    }

    fn activity_import_derived_data_json() -> bike_core::activity_details::StoredActivityDerivedData {
        serialize_derived_activity_data(&ActivityDerivedData {
            recording: Default::default(),
            laps: Vec::new(),
            chart_points: Vec::new(),
            route_points: vec![ActivityRoutePoint {
                elapsed_seconds: 0,
                latitude: 45.523,
                longitude: -122.676,
                distance_meters: Some(0.0),
                elevation_meters: Some(100.0),
                speed_mps: Some(0.0),
                heart_rate_bpm: Some(130),
                cadence_rpm: Some(82),
                power_watts: None,
            }],
        })
        .expect("serialize derived activity data")
    }

    fn activity_import_fixture(now: DateTime<Utc>) -> activity_imports::Model {
        activity_imports::Model {
            id: 7,
            user_id: 12,
            activity_id: Some(99),
            processing_stage: "complete".to_string(),
            processed_at: Some(now),
            last_processing_event_at: Some(now),
            original_filename: "ride.gpx".to_string(),
            storage_path: "activity-imports/user/ride.gpx".to_string(),
            size_bytes: 8192,
            created_at: now,
            updated_at: now,
            ..bike_core::test_support::activity_import()
        }
    }
}
