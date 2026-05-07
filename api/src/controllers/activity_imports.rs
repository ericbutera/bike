use crate::activity_details::{
    derive_activity_detail_data, serialize_derived_activity_data,
};
use crate::activity_lifecycle::refresh_activity_derived_state;
use crate::activity_location::location_from_derived_json;
use crate::activity_summary::summarize_activity_upload;
use crate::app_error::{ApiErrorResponse, AppError};
use crate::config::Config;
use crate::entities::{activities, activity_imports};
use crate::storage::AppStorage;
use crate::training_profile::{
    load_training_profile, serialize_activity_heart_rate_zones,
    summarize_heart_rate_zones,
};
use axum::extract::{Multipart, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use kaleido::auth::UserContext;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use utoipa::ToSchema;
use uuid::Uuid;

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
            activity_duration_seconds: activity.and_then(|value| {
                value.moving_time_seconds.or(value.total_time_seconds)
            }),
            activity_location: activity
                .and_then(|value| location_from_derived_json(value.derived_data_json.as_deref())),
        }
    }
}

struct UploadedActivityFile {
    original_filename: String,
    format: String,
    mime_type: Option<String>,
    bytes: Vec<u8>,
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
                .filter_map(|activity| activity.activity_import_id.map(|import_id| (import_id, activity)))
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
    post,
    path = "/api/activity-imports",
    request_body(content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "Activity import uploaded", body = ActivityImportResponse),
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
    let activity_draft = summarize_activity_upload(
        &upload.original_filename,
        &upload.format,
        &upload.bytes,
    )?;
    let derived_data = derive_activity_detail_data(
        &upload.original_filename,
        &upload.format,
        &upload.bytes,
    )?;
    let training_profile = load_training_profile(&state.db, user.id).await?;
    let heart_rate_zones = summarize_heart_rate_zones(
        &derived_data.route_points,
        &derived_data.chart_points,
        activity_draft.moving_time_seconds.or(activity_draft.total_time_seconds),
        activity_draft.average_heart_rate_bpm,
        training_profile.heart_rate_zone_bounds_bpm.as_deref(),
    );
    let heart_rate_zones_json = serialize_activity_heart_rate_zones(&heart_rate_zones)?;
    let derived_data_json = serialize_derived_activity_data(&derived_data)?;
    let relative_path = format!(
        "activity-imports/{}/{}.{}",
        user.pid,
        Uuid::new_v4(),
        upload.format
    );
    let full_path = Path::new(&state.uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &upload.bytes).await?;

    let original_filename = upload.original_filename.clone();
    let format = upload.format.clone();
    let mime_type = upload.mime_type.clone();
    let size_bytes = upload.bytes.len() as i64;

    let model = activity_imports::ActiveModel {
        user_id: Set(user.id),
        source: Set("manual_upload".to_string()),
        format: Set(format.clone()),
        status: Set("uploaded".to_string()),
        original_filename: Set(original_filename.clone()),
        storage_path: Set(relative_path),
        size_bytes: Set(size_bytes),
        mime_type: Set(mime_type.clone()),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    let activity_model = activities::ActiveModel {
        user_id: Set(user.id),
        activity_import_id: Set(Some(model.id)),
        title: Set(activity_draft.title),
        sport: Set(activity_draft.sport),
        source: Set("manual_upload".to_string()),
        original_filename: Set(Some(original_filename)),
        format: Set(Some(format)),
        started_at: Set(activity_draft.started_at),
        ended_at: Set(activity_draft.ended_at),
        distance_meters: Set(activity_draft.distance_meters),
        moving_time_seconds: Set(activity_draft.moving_time_seconds),
        total_time_seconds: Set(activity_draft.total_time_seconds),
        elevation_gain_meters: Set(activity_draft.elevation_gain_meters),
        elevation_loss_meters: Set(activity_draft.elevation_loss_meters),
        average_speed_mps: Set(activity_draft.average_speed_mps),
        max_speed_mps: Set(activity_draft.max_speed_mps),
        average_heart_rate_bpm: Set(activity_draft.average_heart_rate_bpm),
        max_heart_rate_bpm: Set(activity_draft.max_heart_rate_bpm),
        average_cadence_rpm: Set(activity_draft.average_cadence_rpm),
        max_cadence_rpm: Set(activity_draft.max_cadence_rpm),
        calories: Set(activity_draft.calories),
        estimated_ftp_watts: Set(training_profile.estimated_ftp_watts),
        heart_rate_zones_json: Set(heart_rate_zones_json),
        derived_data_json: Set(Some(derived_data_json)),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    refresh_activity_derived_state(
        &state.db,
        user.id,
        activity_model.id,
        &derived_data.route_points,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ActivityImportResponse::from_model(
            model,
            Some(&activity_model),
        )),
    ))
}

async fn read_uploaded_activity_file(
    mut multipart: Multipart,
) -> Result<UploadedActivityFile, AppError> {
    let max_upload_bytes = Config::get().max_upload_bytes;

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|err| {
            map_multipart_error(
                &err.to_string(),
                max_upload_bytes,
                "Malformed multipart payload",
            )
        })?
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
                    format!(
                        "File exceeds the {} byte upload limit",
                        max_upload_bytes
                    ),
                ));
            }
            bytes.extend_from_slice(&chunk);
        }

        if bytes.is_empty() {
            return Err(AppError::validation_field(
                "file",
                "Uploaded file is empty",
            ));
        }

        return Ok(UploadedActivityFile {
            original_filename,
            format,
            mime_type,
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

fn validate_activity_format(filename: &str) -> Result<String, AppError> {
    let extension = Path::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| {
            AppError::validation_field(
                "file",
                "Uploaded file must include a .fit, .tcx, or .gpx extension",
            )
        })?;

    match extension.as_str() {
        "fit" | "tcx" | "gpx" => Ok(extension),
        _ => Err(AppError::validation_field(
            "file",
            "Only .fit, .tcx, and .gpx uploads are supported",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            error
                .errors
                .unwrap()
                .get("file")
                .unwrap(),
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
        let error = map_multipart_error(
            "missing boundary",
            1024,
            "Malformed multipart payload",
        );

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Malformed multipart payload: missing boundary");
        assert!(error.errors.is_none());
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
        let response = ActivityImportResponse::from_model(activity_imports::Model {
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
        }, Some(&activity));

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