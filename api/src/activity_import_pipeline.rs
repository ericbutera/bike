use crate::activity_details::{derive_activity_detail_data, serialize_derived_activity_data};
use crate::activity_lifecycle::refresh_activity_derived_state;
use crate::analytics::{mark_segment_activity_changes, mark_user_activity_change};
use crate::app_error::AppError;
use crate::dedupe::{activity_dedupe_key, activity_dedupe_key_from_model};
use crate::entities::{activities, activity_imports};
use crate::tasks::TaskQueue;
use crate::training_profile::{
    load_training_profile, serialize_activity_heart_rate_zones, summarize_heart_rate_zones,
    TrainingProfile,
};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ActivityUploadPayload {
    pub original_filename: String,
    pub format: String,
    pub mime_type: Option<String>,
    pub bytes: Vec<u8>,
}

pub struct PersistedActivityImport {
    pub import: activity_imports::Model,
    pub activity: activities::Model,
    pub affected_segment_ids: Vec<i32>,
}

pub struct DeduplicatedActivityImport {
    pub activity: activities::Model,
    pub existing_import: Option<activity_imports::Model>,
}

pub enum PersistActivityUploadOutcome {
    Imported(PersistedActivityImport),
    Duplicate(DeduplicatedActivityImport),
}

pub async fn persist_activity_upload(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_storage_key: &str,
    user_id: i32,
    upload: ActivityUploadPayload,
    source: &str,
    training_profile: Option<&TrainingProfile>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let activity_draft =
        crate::activity_summary::summarize_activity_upload(
            &upload.original_filename,
            &upload.format,
            &upload.bytes,
        )?;
    let derived_data = derive_activity_detail_data(
        &upload.original_filename,
        &upload.format,
        &upload.bytes,
    )?;

    if let Some(duplicate) =
        find_duplicate_activity(db, user_id, &activity_draft, &derived_data).await?
    {
        return Ok(PersistActivityUploadOutcome::Duplicate(duplicate));
    }

    let training_profile = match training_profile {
        Some(profile) => profile.clone(),
        None => load_training_profile(db, user_id).await?,
    };
    let heart_rate_zones = summarize_heart_rate_zones(
        &derived_data.route_points,
        &derived_data.chart_points,
        activity_draft
            .moving_time_seconds
            .or(activity_draft.total_time_seconds),
        activity_draft.average_heart_rate_bpm,
        training_profile.heart_rate_zone_bounds_bpm.as_deref(),
    );
    let heart_rate_zones_json = serialize_activity_heart_rate_zones(&heart_rate_zones)?;
    let derived_data_json = serialize_derived_activity_data(&derived_data)?;
    let relative_path = format!(
        "activity-imports/{}/{}.{}",
        user_storage_key,
        Uuid::new_v4(),
        upload.format
    );
    let full_path = Path::new(uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &upload.bytes).await?;

    let original_filename = upload.original_filename.clone();
    let format = upload.format.clone();
    let mime_type = upload.mime_type.clone();
    let size_bytes = upload.bytes.len() as i64;

    let import_model = activity_imports::ActiveModel {
        user_id: Set(user_id),
        source: Set(source.to_string()),
        format: Set(format.clone()),
        status: Set("uploaded".to_string()),
        original_filename: Set(original_filename.clone()),
        storage_path: Set(relative_path),
        size_bytes: Set(size_bytes),
        mime_type: Set(mime_type.clone()),
        ..Default::default()
    }
    .insert(db)
    .await?;

    let activity_model = activities::ActiveModel {
        user_id: Set(user_id),
        activity_import_id: Set(Some(import_model.id)),
        title: Set(activity_draft.title),
        sport: Set(activity_draft.sport),
        source: Set(source.to_string()),
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
    .insert(db)
    .await?;

    let affected_segment_ids = refresh_activity_derived_state(
        db,
        user_id,
        activity_model.id,
        &derived_data.route_points,
    )
    .await?;

    Ok(PersistActivityUploadOutcome::Imported(PersistedActivityImport {
        import: import_model,
        activity: activity_model,
        affected_segment_ids,
    }))
}

pub async fn finalize_activity_import_batch(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    mut affected_segment_ids: Vec<i32>,
    changed_at: DateTime<Utc>,
) -> Result<(), AppError> {
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    mark_user_activity_change(db, user_id, changed_at).await?;
    if !affected_segment_ids.is_empty() {
        mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    }

    tasks.rebuild_fitness_freshness(user_id).await;
    if !affected_segment_ids.is_empty() {
        tasks.rebuild_segment_analytics(affected_segment_ids).await;
    }

    Ok(())
}

pub fn validate_activity_format(filename: &str) -> Result<String, AppError> {
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

async fn find_duplicate_activity(
    db: &DatabaseConnection,
    user_id: i32,
    activity_draft: &crate::activity_summary::ActivityDraft,
    derived_data: &crate::activity_details::ActivityDerivedData,
) -> Result<Option<DeduplicatedActivityImport>, AppError> {
    let target_key = activity_dedupe_key(activity_draft, &derived_data.route_points);
    let candidates = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::StartedAt.eq(activity_draft.started_at))
        .filter(activities::Column::Sport.eq(activity_draft.sport.clone()))
        .all(db)
        .await?;

    for activity in candidates {
        if activity_dedupe_key_from_model(&activity) == target_key {
            let existing_import = match activity.activity_import_id {
                Some(import_id) => activity_imports::Entity::find_by_id(import_id).one(db).await?,
                None => None,
            };

            return Ok(Some(DeduplicatedActivityImport {
                activity,
                existing_import,
            }));
        }
    }

    Ok(None)
}