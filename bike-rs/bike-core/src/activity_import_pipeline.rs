//! Retained activity files and parser metadata. Temporal calls the processing steps.
use crate::activity_parser::{parse_activity_artifact, ActivityParserArtifact, ParsedActivityData};
use crate::activity_type::ActivityType;
use crate::entities::{activities, activity_import_artifacts, activity_imports};
use crate::integration_events_service::INTEGRATION_LEVEL_INFO;
use crate::object_store::{ObjectRef, ObjectStore};
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ActivityUploadPayload {
    pub original_filename: String,
    pub format: String,
    pub mime_type: Option<String>,
    pub source_correlation_id: Option<String>,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct ActivityImportArtifactPayload {
    pub artifact_kind: String,
    pub format: String,
    pub source_quality: String,
    pub original_filename: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

pub struct StoreActivityUploadImportRequest<'a> {
    pub uploads_dir: &'a str,
    pub user_storage_key: &'a str,
    pub user_id: i32,
    pub upload: ActivityUploadPayload,
    pub primary_artifact_kind: &'a str,
    pub primary_source_quality: &'a str,
    pub additional_artifacts: Vec<ActivityImportArtifactPayload>,
    pub source: &'a str,
}

pub use crate::activity_import_lifecycle::{
    ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT, ACTIVITY_IMPORT_STAGE_ACTIVITY_PARSED,
    ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED, ACTIVITY_IMPORT_STAGE_COMPLETE, ACTIVITY_IMPORT_STAGE_RAW_STORED,
    ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT, ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT,
    ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT, ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS,
    ACTIVITY_IMPORT_STATUS_DUPLICATE, ACTIVITY_IMPORT_STATUS_FAILED, ACTIVITY_IMPORT_STATUS_PROCESSED,
    ACTIVITY_IMPORT_STATUS_PROCESSING, ACTIVITY_PROCESSING_PROVIDER,
};
pub const ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL: &str = "original";
pub const ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD: &str = "provider_payload";
pub const ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT: &str = "generated_export";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL: &str = "fit_original";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_TCX_ORIGINAL: &str = "tcx_original";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_GPX_ORIGINAL: &str = "gpx_original";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS: &str = "strava_streams";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX: &str = "generated_tcx";
pub const ACTIVITY_IMPORT_VERSION_LEGACY: i32 = activity_imports::ACTIVITY_IMPORT_VERSION_LEGACY;
pub const ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE: i32 = activity_imports::ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE;
pub const ACTIVITY_IMPORT_VERSION_CURRENT: i32 = activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT;
fn activity_import_storage_path(user_storage_key: &str, format: &str, bucket_at: DateTime<Utc>) -> String {
    format!(
        "activity-imports/{}/{}/{}.{}",
        user_storage_key,
        bucket_at.format("%Y/%m"),
        Uuid::new_v4(),
        format
    )
}

pub fn infer_activity_type(title: &str, original_filename: &str) -> ActivityType {
    ActivityType::infer(title, original_filename)
}

#[derive(Debug, Clone)]
pub(crate) struct ActivityProcessingArtifact {
    pub(crate) size_bytes: i64,
    pub(crate) checksum_sha256: Option<String>,
    pub(crate) artifact_kind: String,
    pub(crate) original_filename: String,
    pub(crate) format: String,
    pub(crate) source_quality: String,
    pub(crate) storage_path: String,
}

impl ActivityProcessingArtifact {
    pub(crate) fn object_reference(&self) -> Result<ObjectRef, AppError> {
        let sha256 = self.checksum_sha256.clone().unwrap_or_default();
        Ok(ObjectRef {
            path: self.storage_path.clone(),
            sha256,
            bytes: self
                .size_bytes
                .try_into()
                .map_err(|_| AppError::bad_request("Invalid retained source size"))?,
        })
    }

    pub(crate) async fn read(&self, objects: &ObjectStore) -> Result<Vec<u8>, AppError> {
        let reference = self.object_reference()?;
        if !reference.sha256.is_empty() {
            return objects.read(&reference).await;
        }
        // Older imports predate checksum metadata. Keep reading their original
        // files; absence of that optional metadata must not prevent reruns.
        let bytes = objects
            .read_bytes(&reference.path)
            .await?
            .ok_or_else(|| AppError::not_found("Retained source is missing"))?;
        if bytes.len() != reference.bytes {
            return Err(AppError::invalid_data("Retained source size changed"));
        }
        Ok(bytes)
    }
}

pub async fn mark_activity_import_failed(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    error: &AppError,
) -> Result<(), AppError> {
    crate::activity_import_lifecycle::mark_activity_import_failed(db, import, stage, &error.message)
        .await
        .map_err(|error| AppError::internal(error.message))
}

async fn record_activity_processing_event(
    db: &impl sea_orm::ConnectionTrait,
    import: &activity_imports::Model,
    event_type: &str,
    level: &str,
    message: String,
    payload: Option<serde_json::Value>,
) {
    crate::activity_import_lifecycle::record_activity_processing_event(db, import, event_type, level, message, payload)
        .await;
}

pub(crate) async fn load_best_activity_parsing_artifact(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
) -> Result<ActivityProcessingArtifact, AppError> {
    let artifacts = activity_import_artifacts::Entity::for_import(db, import.user_id, import.id).await?;

    artifacts
        .into_iter()
        .filter(activity_artifact_is_parsable)
        .max_by_key(activity_artifact_parse_priority)
        .map(activity_processing_artifact_from_model)
        .or_else(|| legacy_activity_processing_artifact(import))
        .ok_or_else(|| {
            AppError::bad_request(format!(
                "Activity import {} does not have a parsable source artifact",
                import.id
            ))
        })
}

fn activity_processing_artifact_from_model(artifact: activity_import_artifacts::Model) -> ActivityProcessingArtifact {
    ActivityProcessingArtifact {
        size_bytes: artifact.size_bytes,
        checksum_sha256: Some(artifact.checksum_sha256),
        artifact_kind: artifact.artifact_kind,
        original_filename: artifact.original_filename,
        format: artifact.format,
        source_quality: artifact.source_quality,
        storage_path: artifact.storage_path,
    }
}

fn legacy_activity_processing_artifact(import: &activity_imports::Model) -> Option<ActivityProcessingArtifact> {
    if !activity_format_is_parsable(&import.format) || (import.source == "strava_sync" && import.format == "tcx") {
        return None;
    }

    Some(ActivityProcessingArtifact {
        size_bytes: import.size_bytes,
        checksum_sha256: None,
        artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL.to_string(),
        original_filename: import.original_filename.clone(),
        format: import.format.clone(),
        source_quality: original_source_quality_for_format(&import.format).to_string(),
        storage_path: import.storage_path.clone(),
    })
}

fn activity_artifact_is_parsable(artifact: &activity_import_artifacts::Model) -> bool {
    match artifact.artifact_kind.as_str() {
        ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL => {
            activity_format_is_parsable(&artifact.format)
                || (artifact.format == "json" && artifact.source_quality == "strava_archive_json")
        }
        ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD => {
            artifact.source_quality == ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS && artifact.format == "json"
        }
        _ => false,
    }
}

fn activity_format_is_parsable(format: &str) -> bool {
    matches!(format, "fit" | "tcx" | "gpx")
}

fn activity_artifact_parse_priority(artifact: &activity_import_artifacts::Model) -> i32 {
    match (
        artifact.artifact_kind.as_str(),
        artifact.source_quality.as_str(),
        artifact.format.as_str(),
    ) {
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, "strava_archive_json", "json") => 350,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL, _) => 500,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, ACTIVITY_IMPORT_SOURCE_QUALITY_TCX_ORIGINAL, _) => 400,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, ACTIVITY_IMPORT_SOURCE_QUALITY_GPX_ORIGINAL, _) => 300,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, _, "fit") => 490,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, _, "tcx") => 390,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, _, "gpx") => 290,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD, ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS, "json") => 200,
        _ => 0,
    }
}

pub(crate) async fn enrich_recording(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    import: &activity_imports::Model,
    artifact: &ActivityProcessingArtifact,
    activity: Option<&activities::Model>,
    parsed: &mut ParsedActivityData,
) -> Result<(), AppError> {
    parsed
        .derived_data
        .recording
        .merge(crate::activity_recording::RecordingContext::legacy(
            &parsed.draft.sport,
            &import.source,
            &parsed.draft.title,
        ));
    merge_recording_artifacts(db, objects, import, artifact, parsed).await?;
    if let Some(activity) = activity {
        parsed.derived_data.recording.merge(activity.recording_context());
    }
    parsed.derived_data.recording = crate::activity_recording_recovery::resolve_recording(
        db,
        activity.map_or(0, |activity| activity.id),
        import.user_id,
        parsed.draft.started_at,
        &parsed.derived_data,
    )
    .await?;
    Ok(())
}

async fn merge_recording_artifacts(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    import: &activity_imports::Model,
    parsing_artifact: &ActivityProcessingArtifact,
    parsed: &mut ParsedActivityData,
) -> Result<(), AppError> {
    let artifacts = activity_import_artifacts::Entity::for_import(db, import.user_id, import.id).await?;
    for artifact in artifacts
        .into_iter()
        .filter(|a| a.storage_path != parsing_artifact.storage_path && activity_artifact_is_parsable(a))
    {
        let artifact = activity_processing_artifact_from_model(artifact);
        let bytes = artifact.read(objects).await?;
        let other = parse_activity_artifact(ActivityParserArtifact {
            original_filename: &artifact.original_filename,
            format: &artifact.format,
            artifact_kind: &artifact.artifact_kind,
            source_quality: &artifact.source_quality,
            bytes: &bytes,
        })?;
        parsed.derived_data.recording.merge(other.derived_data.recording);
    }
    Ok(())
}

pub async fn find_stored_activity_import(
    db: &DatabaseConnection,
    user_id: i32,
    bytes: &[u8],
) -> Result<Option<activity_imports::Model>, AppError> {
    activity_import_artifacts::Entity::find_existing_original_by_checksum(db, user_id, &checksum_sha256_hex(bytes))
        .await
        .map_err(AppError::from)
}

pub async fn store_activity_upload_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_storage_key: &str,
    user_id: i32,
    upload: ActivityUploadPayload,
    source: &str,
) -> Result<activity_imports::Model, AppError> {
    let source_quality = original_source_quality_for_format(&upload.format);
    store_activity_upload_import_with_artifacts(
        db,
        StoreActivityUploadImportRequest {
            uploads_dir,
            user_storage_key,
            user_id,
            upload,
            primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
            primary_source_quality: source_quality,
            additional_artifacts: Vec::new(),
            source,
        },
    )
    .await
}

pub async fn store_activity_upload_import_with_artifacts(
    db: &impl sea_orm::ConnectionTrait,
    request: StoreActivityUploadImportRequest<'_>,
) -> Result<activity_imports::Model, AppError> {
    let relative_path = store_activity_upload_source(&request).await?;
    let StoreActivityUploadImportRequest {
        uploads_dir,
        user_storage_key,
        user_id,
        upload,
        primary_artifact_kind,
        primary_source_quality,
        additional_artifacts,
        source,
    } = request;
    let primary_artifact = ActivityImportArtifactPayload {
        artifact_kind: primary_artifact_kind.to_string(),
        format: upload.format.clone(),
        source_quality: primary_source_quality.to_string(),
        original_filename: upload.original_filename.clone(),
        mime_type: upload.mime_type.clone(),
        bytes: upload.bytes.clone(),
    };

    let import_model =
        activity_imports::Model::store_upload(db, user_id, source, &upload, relative_path.clone()).await?;

    insert_activity_import_artifact(
        db,
        &import_model,
        primary_artifact,
        relative_path,
        upload.bytes.len() as i64,
    )
    .await?;

    for artifact in additional_artifacts {
        store_additional_activity_import_artifact(db, uploads_dir, user_storage_key, &import_model, artifact).await?;
    }

    record_activity_processing_event(
        db,
        &import_model,
        "stage_completed",
        INTEGRATION_LEVEL_INFO,
        format!("Activity import {} stored raw source", import_model.id),
        Some(serde_json::json!({
            "import_id": import_model.id,
            "source": import_model.source,
            "stage": ACTIVITY_IMPORT_STAGE_RAW_STORED,
        })),
    )
    .await;

    Ok(import_model)
}

async fn store_activity_upload_source(request: &StoreActivityUploadImportRequest<'_>) -> Result<String, AppError> {
    if request.source == "strava_sync"
        && request.primary_artifact_kind == ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD
    {
        return Ok(crate::strava::objects::ObjectStore::new(request.uploads_dir)
            .write(&format!("strava/sources/{}", request.user_id), &request.upload.bytes)
            .await?
            .path);
    }
    let relative = activity_import_storage_path(request.user_storage_key, &request.upload.format, Utc::now());
    let full_path = Path::new(request.uploads_dir).join(&relative);
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(full_path, &request.upload.bytes).await?;
    Ok(relative)
}

pub fn original_source_quality_for_format(format: &str) -> &'static str {
    match format {
        "fit" => ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
        "tcx" => ACTIVITY_IMPORT_SOURCE_QUALITY_TCX_ORIGINAL,
        "gpx" => ACTIVITY_IMPORT_SOURCE_QUALITY_GPX_ORIGINAL,
        _ => "unknown_original",
    }
}

async fn store_additional_activity_import_artifact(
    db: &impl sea_orm::ConnectionTrait,
    uploads_dir: &str,
    user_storage_key: &str,
    import: &activity_imports::Model,
    artifact: ActivityImportArtifactPayload,
) -> Result<activity_import_artifacts::Model, AppError> {
    let relative_path = activity_import_storage_path(user_storage_key, &artifact.format, Utc::now());
    let full_path = Path::new(uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &artifact.bytes).await?;
    let size_bytes = artifact.bytes.len() as i64;
    insert_activity_import_artifact(db, import, artifact, relative_path, size_bytes).await
}

async fn insert_activity_import_artifact(
    db: &impl sea_orm::ConnectionTrait,
    import: &activity_imports::Model,
    artifact: ActivityImportArtifactPayload,
    storage_path: String,
    size_bytes: i64,
) -> Result<activity_import_artifacts::Model, AppError> {
    activity_import_artifacts::Model::store_artifact(db, import, artifact, storage_path, size_bytes).await
}

pub(crate) fn checksum_sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn validate_activity_format(filename: &str) -> Result<String, AppError> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            AppError::validation_field("file", "Uploaded file must include a .fit, .tcx, or .gpx extension")
        })?;

    match extension.as_str() {
        "fit" | "tcx" | "gpx" => Ok(extension),
        _ => Err(AppError::validation_field(
            "file",
            "Only .fit, .tcx, and .gpx uploads are supported",
        )),
    }
}
