use crate::activity_details::{derive_activity_detail_data, serialize_derived_activity_data};
use crate::activity_lifecycle::{
    refresh_activity_derived_state, refresh_activity_derived_state_without_cache_rebuilds,
};
use crate::activity_training_analysis::rebuild_activity_training_analysis_cache;
use crate::activity_type::ActivityType;
use crate::analytics::{
    mark_segment_activity_changes, mark_user_activity_change, mark_user_fitness_dirty,
    rebuild_activity_analytics_cache, rebuild_segment_analytics_cache,
};
use crate::app_error::AppError;
use crate::dedupe::activity_dedupe_matches_model;
use crate::entities::{activities, activity_imports};
use crate::integration_events::{
    record_event, NewIntegrationEvent, INTEGRATION_LEVEL_ERROR, INTEGRATION_LEVEL_INFO,
    INTEGRATION_LEVEL_SUCCESS,
};
use crate::tasks::{ProcessActivityImportTask, TaskQueue};
use crate::training_profile::{
    load_training_profile, serialize_activity_heart_rate_zones, summarize_heart_rate_zones,
    TrainingProfile,
};
use chrono::{DateTime, Duration as ChronoDuration, NaiveDate, Utc};
use kaleido::background_jobs::background_tasks;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
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

pub struct PersistedActivityImport {
    pub import: activity_imports::Model,
    pub activity: activities::Model,
    pub affected_segment_ids: Vec<i32>,
    pub fitness_dirty_from_day: NaiveDate,
}

pub struct ReprocessedActivityImport {
    pub activity: activities::Model,
    pub affected_segment_ids: Vec<i32>,
    pub fitness_dirty_from_day: NaiveDate,
}

pub struct DeduplicatedActivityImport {
    pub activity: activities::Model,
    pub existing_import: Option<activity_imports::Model>,
}

pub enum PersistActivityUploadOutcome {
    Imported(PersistedActivityImport),
    Duplicate(DeduplicatedActivityImport),
}

pub const ACTIVITY_IMPORT_STATUS_PROCESSING: &str = "processing";
pub const ACTIVITY_IMPORT_STATUS_PROCESSED: &str = "processed";
pub const ACTIVITY_IMPORT_STATUS_FAILED: &str = "failed";
pub const ACTIVITY_IMPORT_STATUS_DUPLICATE: &str = "duplicate";
pub const ACTIVITY_IMPORT_STAGE_RAW_STORED: &str = "raw_stored";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED: &str = "activity_saved";
pub const ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT: &str = "segments_built";
pub const ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT: &str = "segment_analytics_built";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT: &str = "activity_analytics_built";
pub const ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT: &str = "training_analysis_built";
pub const ACTIVITY_IMPORT_STAGE_COMPLETE: &str = "complete";
pub const ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS: i64 = 300;
const ACTIVITY_PROCESSING_PROVIDER: &str = "activity_processing";
const PROCESS_ACTIVITY_IMPORT_TASK_TYPE: &str = "process_activity_import";
const MANUAL_UPLOAD_SOURCE: &str = "manual_upload";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityUploadDeduplication {
    Enabled,
    Disabled,
}

fn activity_import_storage_path(
    user_storage_key: &str,
    format: &str,
    bucket_at: DateTime<Utc>,
) -> String {
    format!(
        "activity-imports/{}/{}/{}.{}",
        user_storage_key,
        bucket_at.format("%Y/%m"),
        Uuid::new_v4(),
        format
    )
}

pub async fn recover_stale_manual_activity_imports(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    let stale_before = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS);
    let user_ids = activity_imports::Entity::find()
        .select_only()
        .column(activity_imports::Column::UserId)
        .distinct()
        .filter(activity_imports::Column::Source.eq(MANUAL_UPLOAD_SOURCE))
        .filter(activity_imports::Column::Status.eq(ACTIVITY_IMPORT_STATUS_PROCESSING))
        .filter(activity_imports::Column::LastProcessingEventAt.lte(stale_before))
        .into_tuple::<i32>()
        .all(db)
        .await?;

    let mut recovered_count = 0usize;
    for user_id in user_ids {
        recovered_count +=
            recover_stale_manual_activity_imports_for_user(db, tasks, user_id, now).await?;
    }

    Ok(recovered_count)
}

pub async fn recover_abandoned_manual_activity_imports_after_worker_start(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    let user_ids = activity_imports::Entity::find()
        .select_only()
        .column(activity_imports::Column::UserId)
        .distinct()
        .filter(activity_imports::Column::Source.eq(MANUAL_UPLOAD_SOURCE))
        .filter(activity_imports::Column::Status.eq(ACTIVITY_IMPORT_STATUS_PROCESSING))
        .into_tuple::<i32>()
        .all(db)
        .await?;

    let mut recovered_count = 0usize;
    for user_id in user_ids {
        recovered_count +=
            recover_manual_activity_imports_for_user(db, tasks, user_id, now, None).await?;
    }

    Ok(recovered_count)
}

pub async fn recover_stale_manual_activity_imports_for_user(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    let stale_before = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS);
    recover_manual_activity_imports_for_user(db, tasks, user_id, now, Some(stale_before)).await
}

async fn recover_manual_activity_imports_for_user(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    now: DateTime<Utc>,
    stale_before: Option<DateTime<Utc>>,
) -> Result<usize, AppError> {
    let imports = activity_imports::Entity::find()
        .filter(activity_imports::Column::UserId.eq(user_id))
        .filter(activity_imports::Column::Source.eq(MANUAL_UPLOAD_SOURCE))
        .filter(activity_imports::Column::Status.eq(ACTIVITY_IMPORT_STATUS_PROCESSING))
        .order_by_asc(activity_imports::Column::CreatedAt)
        .all(db)
        .await?;

    let mut recovered_count = 0usize;

    for import in imports {
        if let Some(stale_before) = stale_before {
            let last_event_at = import.last_processing_event_at.unwrap_or(import.updated_at);
            if last_event_at > stale_before {
                continue;
            }
        }

        let active_tasks =
            find_active_process_activity_import_tasks(db, user_id, import.id).await?;
        let has_fresh_processing_task = active_tasks.iter().any(|task| {
            stale_before.is_some_and(|stale_before| {
                task.status == background_tasks::TaskStatus::Processing.as_str()
                    && task.updated_at > stale_before
            })
        });

        if has_fresh_processing_task {
            continue;
        }

        let mut recovered_this_import = false;
        for task in active_tasks
            .iter()
            .filter(|task| task.status == background_tasks::TaskStatus::Processing.as_str())
        {
            reset_background_task_to_pending(db, task).await?;
            recovered_this_import = true;
        }

        let has_pending_task = active_tasks
            .iter()
            .any(|task| task.status == background_tasks::TaskStatus::Pending.as_str());

        if !has_pending_task && !recovered_this_import {
            tasks
                .process_activity_import(user_id, import.id)
                .await
                .map_err(|message| {
                    AppError::internal(format!(
                        "Failed to requeue stale activity import {}: {message}",
                        import.id
                    ))
                })?;
            recovered_this_import = true;
        }

        if has_pending_task || recovered_this_import {
            mark_activity_import_requeued(db, &import, now).await?;
            recovered_count += 1;
        }
    }

    Ok(recovered_count)
}

async fn find_active_process_activity_import_tasks(
    db: &DatabaseConnection,
    user_id: i32,
    import_id: i32,
) -> Result<Vec<background_tasks::Model>, AppError> {
    let tasks = background_tasks::Entity::find()
        .filter(background_tasks::Column::TaskType.eq(PROCESS_ACTIVITY_IMPORT_TASK_TYPE))
        .filter(background_tasks::Column::Status.is_in([
            background_tasks::TaskStatus::Pending.as_str(),
            background_tasks::TaskStatus::Processing.as_str(),
        ]))
        .order_by_desc(background_tasks::Column::CreatedAt)
        .all(db)
        .await?;

    Ok(tasks
        .into_iter()
        .filter(|task| task_targets_activity_import(task, user_id, import_id))
        .collect())
}

fn task_targets_activity_import(
    task: &background_tasks::Model,
    user_id: i32,
    import_id: i32,
) -> bool {
    serde_json::from_value::<ProcessActivityImportTask>(
        task.payload
            .get("data")
            .cloned()
            .unwrap_or_else(|| task.payload.clone()),
    )
    .map(|task| task.user_id == user_id && task.import_id == import_id)
    .unwrap_or(false)
}

async fn reset_background_task_to_pending(
    db: &DatabaseConnection,
    task: &background_tasks::Model,
) -> Result<(), AppError> {
    let mut active: background_tasks::ActiveModel = task.clone().into();
    active.status = Set(background_tasks::TaskStatus::Pending.as_str().to_string());
    active.attempts = Set(0);
    active.error = Set(None);
    active.scheduled_for = Set(None);
    active.started_at = Set(None);
    active.completed_at = Set(None);
    active.result = Set(None);
    active.updated_at = Set(Utc::now());
    active.update(db).await?;

    Ok(())
}

async fn mark_activity_import_requeued(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    now: DateTime<Utc>,
) -> Result<activity_imports::Model, AppError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string());
    active_model.processing_stage = Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.to_string());
    active_model.processing_error = Set(None);
    active_model.last_processing_event_at = Set(Some(now));
    active_model.update(db).await.map_err(AppError::from)
}

pub fn infer_activity_type(title: &str, original_filename: &str) -> ActivityType {
    let haystack = format!("{title} {original_filename}").to_ascii_lowercase();

    if haystack
        .split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| matches!(token, "race" | "raceday" | "result" | "results"))
    {
        ActivityType::Race
    } else {
        ActivityType::Training
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReprocessCacheRefresh {
    Immediate,
    Deferred,
}

pub async fn mark_activity_import_processing_stage(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    activity_id: Option<i32>,
) -> Result<activity_imports::Model, AppError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string());
    active_model.processing_stage = Set(stage.to_string());
    active_model.processing_error = Set(None);
    active_model.last_processing_event_at = Set(Some(Utc::now()));
    if let Some(activity_id) = activity_id {
        active_model.activity_id = Set(Some(activity_id));
    }

    let updated = active_model.update(db).await?;
    record_activity_processing_event(
        db,
        &updated,
        "stage_completed",
        INTEGRATION_LEVEL_INFO,
        format!("Activity import {} reached {stage}", updated.id),
        Some(serde_json::json!({
            "import_id": updated.id,
            "activity_id": updated.activity_id,
            "source": updated.source,
            "stage": stage,
        })),
    )
    .await;

    Ok(updated)
}

pub async fn mark_activity_imports_processed(
    db: &DatabaseConnection,
    import_ids: &[i32],
) -> Result<(), AppError> {
    let mut import_ids = import_ids
        .iter()
        .copied()
        .filter(|import_id| *import_id > 0)
        .collect::<Vec<_>>();
    import_ids.sort_unstable();
    import_ids.dedup();

    if import_ids.is_empty() {
        return Ok(());
    }

    let imports = activity_imports::Entity::find()
        .filter(activity_imports::Column::Id.is_in(import_ids.iter().copied()))
        .all(db)
        .await?;

    for import in imports {
        let mut active_model: activity_imports::ActiveModel = import.into();
        active_model.status = Set(ACTIVITY_IMPORT_STATUS_PROCESSED.to_string());
        active_model.processing_stage = Set(ACTIVITY_IMPORT_STAGE_COMPLETE.to_string());
        active_model.processing_error = Set(None);
        active_model.processed_at = Set(Some(Utc::now()));
        active_model.last_processing_event_at = Set(Some(Utc::now()));
        let updated = active_model.update(db).await?;
        record_activity_processing_event(
            db,
            &updated,
            "import_processed",
            INTEGRATION_LEVEL_SUCCESS,
            format!("Activity import {} completed processing", updated.id),
            Some(serde_json::json!({
                "import_id": updated.id,
                "activity_id": updated.activity_id,
                "source": updated.source,
                "stage": ACTIVITY_IMPORT_STAGE_COMPLETE,
            })),
        )
        .await;
    }

    Ok(())
}

pub async fn mark_activity_import_failed(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    error: &AppError,
) -> Result<(), AppError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_FAILED.to_string());
    active_model.processing_stage = Set(stage.to_string());
    active_model.processing_error = Set(Some(error.message.clone()));
    active_model.processing_attempts = Set(import.processing_attempts.saturating_add(1));
    active_model.last_processing_event_at = Set(Some(Utc::now()));
    let updated = active_model.update(db).await?;

    record_activity_processing_event(
        db,
        &updated,
        "import_failed",
        INTEGRATION_LEVEL_ERROR,
        format!(
            "Activity import {} failed at {stage}: {}",
            updated.id, error.message
        ),
        Some(serde_json::json!({
            "import_id": updated.id,
            "activity_id": updated.activity_id,
            "source": updated.source,
            "stage": stage,
            "error": error.message,
        })),
    )
    .await;

    Ok(())
}

pub async fn mark_activity_import_duplicate(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    duplicate_activity_id: i32,
) -> Result<activity_imports::Model, AppError> {
    let mut active_model: activity_imports::ActiveModel = import.clone().into();
    active_model.status = Set(ACTIVITY_IMPORT_STATUS_DUPLICATE.to_string());
    active_model.activity_id = Set(Some(duplicate_activity_id));
    active_model.processing_stage = Set(ACTIVITY_IMPORT_STAGE_COMPLETE.to_string());
    active_model.processing_error = Set(None);
    active_model.processed_at = Set(Some(Utc::now()));
    active_model.last_processing_event_at = Set(Some(Utc::now()));
    let updated = active_model.update(db).await?;

    record_activity_processing_event(
        db,
        &updated,
        "import_duplicate",
        INTEGRATION_LEVEL_INFO,
        format!(
            "Activity import {} matched existing activity {}",
            updated.id, duplicate_activity_id
        ),
        Some(serde_json::json!({
            "import_id": updated.id,
            "activity_id": duplicate_activity_id,
            "source": updated.source,
            "stage": ACTIVITY_IMPORT_STAGE_COMPLETE,
        })),
    )
    .await;

    Ok(updated)
}

async fn record_activity_processing_event(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    event_type: &str,
    level: &str,
    message: String,
    payload: Option<serde_json::Value>,
) {
    if let Err(error) = record_event(
        db,
        NewIntegrationEvent {
            user_id: Some(import.user_id),
            provider: ACTIVITY_PROCESSING_PROVIDER.to_string(),
            event_type: event_type.to_string(),
            level: level.to_string(),
            message,
            connection_id: None,
            payload,
        },
    )
    .await
    {
        tracing::warn!(
            import_id = import.id,
            error = %error.message,
            "failed to record activity processing event"
        );
    }
}

pub async fn persist_activity_upload(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_storage_key: &str,
    user_id: i32,
    upload: ActivityUploadPayload,
    source: &str,
    deduplication: ActivityUploadDeduplication,
    training_profile: Option<&TrainingProfile>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let source_correlation_id = upload.source_correlation_id.clone();

    if deduplication == ActivityUploadDeduplication::Enabled {
        if let Some(existing) = find_existing_activity_by_source_correlation(
            db,
            user_id,
            source,
            source_correlation_id.as_deref(),
        )
        .await?
        {
            return Ok(PersistActivityUploadOutcome::Duplicate(existing));
        }
    }

    let activity_draft = crate::activity_summary::summarize_activity_upload(
        &upload.original_filename,
        &upload.format,
        &upload.bytes,
    )?;
    let derived_data =
        derive_activity_detail_data(&upload.original_filename, &upload.format, &upload.bytes)?;

    if deduplication == ActivityUploadDeduplication::Enabled {
        if let Some(duplicate) =
            find_duplicate_activity(db, user_id, &activity_draft, &derived_data).await?
        {
            return Ok(PersistActivityUploadOutcome::Duplicate(duplicate));
        }
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
    let relative_path =
        activity_import_storage_path(user_storage_key, &upload.format, activity_draft.started_at);
    let full_path = Path::new(uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &upload.bytes).await?;

    let original_filename = upload.original_filename.clone();
    let activity_type = infer_activity_type(&activity_draft.title, &original_filename);
    let format = upload.format.clone();
    let mime_type = upload.mime_type.clone();
    let size_bytes = upload.bytes.len() as i64;

    let import_model = activity_imports::ActiveModel {
        user_id: Set(user_id),
        source: Set(source.to_string()),
        format: Set(format.clone()),
        status: Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string()),
        activity_id: Set(None),
        processing_stage: Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.to_string()),
        processing_error: Set(None),
        processing_attempts: Set(0),
        processed_at: Set(None),
        last_processing_event_at: Set(Some(Utc::now())),
        original_filename: Set(original_filename.clone()),
        storage_path: Set(relative_path),
        size_bytes: Set(size_bytes),
        mime_type: Set(mime_type.clone()),
        ..Default::default()
    }
    .insert(db)
    .await?;
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

    let activity_model = activities::ActiveModel {
        user_id: Set(user_id),
        activity_import_id: Set(Some(import_model.id)),
        title: Set(activity_draft.title),
        sport: Set(activity_draft.sport),
        source: Set(source.to_string()),
        source_correlation_id: Set(source_correlation_id),
        original_filename: Set(Some(original_filename)),
        format: Set(Some(format)),
        activity_type: Set(activity_type.as_str().to_string()),
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

    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED,
        Some(activity_model.id),
    )
    .await?;

    let affected_segment_ids = refresh_activity_derived_state_without_cache_rebuilds(
        db,
        user_id,
        activity_model.id,
        &derived_data.route_points,
    )
    .await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    rebuild_segment_analytics_cache(db, &affected_segment_ids).await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    rebuild_activity_analytics_cache(db, &[activity_model.id]).await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    rebuild_activity_training_analysis_cache(db, &[activity_model.id]).await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    Ok(PersistActivityUploadOutcome::Imported(
        PersistedActivityImport {
            import: import_model,
            activity: activity_model,
            affected_segment_ids,
            fitness_dirty_from_day: activity_draft.started_at.date_naive(),
        },
    ))
}

pub async fn store_activity_upload_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_storage_key: &str,
    user_id: i32,
    upload: ActivityUploadPayload,
    source: &str,
) -> Result<activity_imports::Model, AppError> {
    let relative_path = activity_import_storage_path(user_storage_key, &upload.format, Utc::now());
    let full_path = Path::new(uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &upload.bytes).await?;

    let import_model = activity_imports::ActiveModel {
        user_id: Set(user_id),
        source: Set(source.to_string()),
        format: Set(upload.format),
        status: Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string()),
        activity_id: Set(None),
        processing_stage: Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.to_string()),
        processing_error: Set(None),
        processing_attempts: Set(0),
        processed_at: Set(None),
        last_processing_event_at: Set(Some(Utc::now())),
        original_filename: Set(upload.original_filename),
        storage_path: Set(relative_path),
        size_bytes: Set(upload.bytes.len() as i64),
        mime_type: Set(upload.mime_type),
        ..Default::default()
    }
    .insert(db)
    .await?;

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

pub async fn process_stored_activity_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    import: activity_imports::Model,
    deduplication: ActivityUploadDeduplication,
    training_profile: Option<&TrainingProfile>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let bytes = tokio::fs::read(Path::new(uploads_dir).join(&import.storage_path)).await?;
    let activity_draft = crate::activity_summary::summarize_activity_upload(
        &import.original_filename,
        &import.format,
        &bytes,
    )?;
    let derived_data =
        derive_activity_detail_data(&import.original_filename, &import.format, &bytes)?;

    if deduplication == ActivityUploadDeduplication::Enabled {
        if let Some(duplicate) =
            find_duplicate_activity(db, user_id, &activity_draft, &derived_data).await?
        {
            let import = mark_activity_import_duplicate(db, &import, duplicate.activity.id).await?;
            return Ok(PersistActivityUploadOutcome::Duplicate(
                DeduplicatedActivityImport {
                    activity: duplicate.activity,
                    existing_import: Some(import),
                },
            ));
        }
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
    let activity_type = infer_activity_type(&activity_draft.title, &import.original_filename);

    let activity_model = activities::ActiveModel {
        user_id: Set(user_id),
        activity_import_id: Set(Some(import.id)),
        title: Set(activity_draft.title),
        sport: Set(activity_draft.sport),
        source: Set(import.source.clone()),
        source_correlation_id: Set(None),
        original_filename: Set(Some(import.original_filename.clone())),
        format: Set(Some(import.format.clone())),
        activity_type: Set(activity_type.as_str().to_string()),
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

    let import_model = mark_activity_import_processing_stage(
        db,
        &import,
        ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED,
        Some(activity_model.id),
    )
    .await?;

    let affected_segment_ids = refresh_activity_derived_state_without_cache_rebuilds(
        db,
        user_id,
        activity_model.id,
        &derived_data.route_points,
    )
    .await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    rebuild_segment_analytics_cache(db, &affected_segment_ids).await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    rebuild_activity_analytics_cache(db, &[activity_model.id]).await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    rebuild_activity_training_analysis_cache(db, &[activity_model.id]).await?;
    let import_model = mark_activity_import_processing_stage(
        db,
        &import_model,
        ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT,
        Some(activity_model.id),
    )
    .await?;

    Ok(PersistActivityUploadOutcome::Imported(
        PersistedActivityImport {
            import: import_model,
            activity: activity_model,
            affected_segment_ids,
            fitness_dirty_from_day: activity_draft.started_at.date_naive(),
        },
    ))
}

pub async fn finalize_activity_import_batch(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    mut affected_segment_ids: Vec<i32>,
    fitness_dirty_from_day: Option<NaiveDate>,
    changed_at: DateTime<Utc>,
) -> Result<(), AppError> {
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    if let Some(dirty_from_day) = fitness_dirty_from_day {
        mark_user_fitness_dirty(db, user_id, dirty_from_day, changed_at).await?;
    } else {
        mark_user_activity_change(db, user_id, changed_at).await?;
    }
    if !affected_segment_ids.is_empty() {
        mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    }

    tasks.rebuild_fitness_freshness(user_id).await;

    Ok(())
}

async fn reprocess_activity_from_import_with_cache_refresh(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    activity_import: activity_imports::Model,
    training_profile: Option<&TrainingProfile>,
    cache_refresh: ReprocessCacheRefresh,
) -> Result<ReprocessedActivityImport, AppError> {
    let bytes = tokio::fs::read(Path::new(uploads_dir).join(&activity_import.storage_path)).await?;
    let activity_draft = crate::activity_summary::summarize_activity_upload(
        &activity_import.original_filename,
        &activity_import.format,
        &bytes,
    )?;
    let derived_data = derive_activity_detail_data(
        &activity_import.original_filename,
        &activity_import.format,
        &bytes,
    )?;
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

    let mut active_model: activities::ActiveModel = activity.into();
    let activity_type =
        infer_activity_type(&activity_draft.title, &activity_import.original_filename);
    active_model.title = Set(activity_draft.title);
    active_model.sport = Set(activity_draft.sport);
    active_model.original_filename = Set(Some(activity_import.original_filename.clone()));
    active_model.format = Set(Some(activity_import.format.clone()));
    active_model.activity_type = Set(activity_type.as_str().to_string());
    active_model.started_at = Set(activity_draft.started_at);
    active_model.ended_at = Set(activity_draft.ended_at);
    active_model.distance_meters = Set(activity_draft.distance_meters);
    active_model.moving_time_seconds = Set(activity_draft.moving_time_seconds);
    active_model.total_time_seconds = Set(activity_draft.total_time_seconds);
    active_model.elevation_gain_meters = Set(activity_draft.elevation_gain_meters);
    active_model.elevation_loss_meters = Set(activity_draft.elevation_loss_meters);
    active_model.average_speed_mps = Set(activity_draft.average_speed_mps);
    active_model.max_speed_mps = Set(activity_draft.max_speed_mps);
    active_model.average_heart_rate_bpm = Set(activity_draft.average_heart_rate_bpm);
    active_model.max_heart_rate_bpm = Set(activity_draft.max_heart_rate_bpm);
    active_model.average_cadence_rpm = Set(activity_draft.average_cadence_rpm);
    active_model.max_cadence_rpm = Set(activity_draft.max_cadence_rpm);
    active_model.calories = Set(activity_draft.calories);
    active_model.estimated_ftp_watts = Set(training_profile.estimated_ftp_watts);
    active_model.heart_rate_zones_json = Set(heart_rate_zones_json);
    active_model.derived_data_json = Set(Some(derived_data_json));

    let updated = active_model.update(db).await?;
    let affected_segment_ids = match cache_refresh {
        ReprocessCacheRefresh::Immediate => {
            refresh_activity_derived_state(db, user_id, updated.id, &derived_data.route_points)
                .await?
        }
        ReprocessCacheRefresh::Deferred => {
            refresh_activity_derived_state_without_cache_rebuilds(
                db,
                user_id,
                updated.id,
                &derived_data.route_points,
            )
            .await?
        }
    };

    if cache_refresh == ReprocessCacheRefresh::Immediate {
        rebuild_activity_training_analysis_cache(db, &[updated.id]).await?;
    }

    Ok(ReprocessedActivityImport {
        activity: updated,
        affected_segment_ids,
        fitness_dirty_from_day: activity_draft.started_at.date_naive(),
    })
}

pub async fn reprocess_activity_from_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    activity_import: activity_imports::Model,
    training_profile: Option<&TrainingProfile>,
) -> Result<ReprocessedActivityImport, AppError> {
    reprocess_activity_from_import_with_cache_refresh(
        db,
        uploads_dir,
        user_id,
        activity,
        activity_import,
        training_profile,
        ReprocessCacheRefresh::Immediate,
    )
    .await
}

pub async fn reprocess_activity_from_import_deferred_caches(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    activity_import: activity_imports::Model,
    training_profile: Option<&TrainingProfile>,
) -> Result<ReprocessedActivityImport, AppError> {
    reprocess_activity_from_import_with_cache_refresh(
        db,
        uploads_dir,
        user_id,
        activity,
        activity_import,
        training_profile,
        ReprocessCacheRefresh::Deferred,
    )
    .await
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
    let candidates = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::StartedAt.eq(activity_draft.started_at))
        .filter(activities::Column::Sport.eq(activity_draft.sport.clone()))
        .all(db)
        .await?;

    for activity in candidates {
        if activity_dedupe_matches_model(&activity, activity_draft, &derived_data.route_points) {
            return deduplicated_activity_import_for_model(db, activity)
                .await
                .map(Some);
        }
    }

    Ok(None)
}

async fn find_existing_activity_by_source_correlation(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    source_correlation_id: Option<&str>,
) -> Result<Option<DeduplicatedActivityImport>, AppError> {
    let Some(source_correlation_id) = source_correlation_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let activity = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Source.eq(source.to_string()))
        .filter(activities::Column::SourceCorrelationId.eq(source_correlation_id.to_string()))
        .one(db)
        .await?;

    match activity {
        Some(activity) => deduplicated_activity_import_for_model(db, activity)
            .await
            .map(Some),
        None => Ok(None),
    }
}

async fn deduplicated_activity_import_for_model(
    db: &DatabaseConnection,
    activity: activities::Model,
) -> Result<DeduplicatedActivityImport, AppError> {
    let existing_import = match activity.activity_import_id {
        Some(import_id) => {
            activity_imports::Entity::find_by_id(import_id)
                .one(db)
                .await?
        }
        None => None,
    };

    Ok(DeduplicatedActivityImport {
        activity,
        existing_import,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{
        activities, activity_analytics, activity_imports, activity_training_analyses,
        segment_efforts, segments,
    };
    use crate::tasks::{ProcessActivityImportTask, Task};
    use crate::training_profile::TrainingProfile;
    use sea_orm::{
        ColumnTrait, ConnectionTrait, Database, EntityTrait, PaginatorTrait, QueryFilter, Schema,
    };

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
        db.execute(&schema.create_table_from_entity(background_tasks::Entity))
            .await
            .expect("create background tasks table");
        db.execute(&schema.create_table_from_entity(activity_analytics::Entity))
            .await
            .expect("create activity analytics table");
        db.execute(&schema.create_table_from_entity(activity_training_analyses::Entity))
            .await
            .expect("create activity training analyses table");
        db.execute(&schema.create_table_from_entity(segments::Entity))
            .await
            .expect("create segments table");
        db.execute(&schema.create_table_from_entity(segment_efforts::Entity))
            .await
            .expect("create segment efforts table");

        db
    }

    fn test_uploads_dir() -> String {
        let uploads_dir =
            std::env::temp_dir().join(format!("bike-activity-import-pipeline-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&uploads_dir).expect("create uploads dir");
        uploads_dir.display().to_string()
    }

    fn fit_upload(source_correlation_id: Option<&str>) -> ActivityUploadPayload {
        ActivityUploadPayload {
            original_filename: "activity.fit".to_string(),
            format: "fit".to_string(),
            mime_type: Some("application/octet-stream".to_string()),
            source_correlation_id: source_correlation_id.map(str::to_string),
            bytes: include_bytes!("../tests/fixtures/activity.fit").to_vec(),
        }
    }

    #[test]
    fn activity_import_storage_path_buckets_files_by_month() {
        let bucket_at = DateTime::parse_from_rfc3339("2026-07-13T12:00:00Z")
            .expect("parse date")
            .with_timezone(&Utc);
        let path = activity_import_storage_path("test-user", "fit", bucket_at);

        assert!(path.starts_with("activity-imports/test-user/2026/07/"));
        assert!(path.ends_with(".fit"));
        assert_eq!(path.split('/').count(), 5);
    }

    async fn insert_processing_import(
        db: &DatabaseConnection,
        import_id_activity_id: Option<i32>,
        stage: &str,
        last_event_at: DateTime<Utc>,
    ) -> activity_imports::Model {
        activity_imports::ActiveModel {
            user_id: Set(1),
            source: Set(MANUAL_UPLOAD_SOURCE.to_string()),
            format: Set("gpx".to_string()),
            status: Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string()),
            activity_id: Set(import_id_activity_id),
            processing_stage: Set(stage.to_string()),
            processing_error: Set(None),
            processing_attempts: Set(0),
            processed_at: Set(None),
            last_processing_event_at: Set(Some(last_event_at)),
            original_filename: Set("stale.gpx".to_string()),
            storage_path: Set("activity-imports/test/stale.gpx".to_string()),
            size_bytes: Set(123),
            mime_type: Set(None),
            created_at: Set(last_event_at),
            updated_at: Set(last_event_at),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert processing import")
    }

    async fn insert_process_activity_import_task(
        db: &DatabaseConnection,
        import_id: i32,
        status: &str,
        updated_at: DateTime<Utc>,
    ) -> background_tasks::Model {
        background_tasks::ActiveModel {
            task_type: Set(PROCESS_ACTIVITY_IMPORT_TASK_TYPE.to_string()),
            payload: Set(serde_json::to_value(Task::ProcessActivityImport(
                ProcessActivityImportTask {
                    user_id: 1,
                    import_id,
                },
            ))
            .expect("serialize process import task")),
            status: Set(status.to_string()),
            attempts: Set(
                if status == background_tasks::TaskStatus::Processing.as_str() {
                    1
                } else {
                    0
                },
            ),
            max_attempts: Set(1),
            error: Set(None),
            scheduled_for: Set(None),
            created_at: Set(updated_at),
            updated_at: Set(updated_at),
            started_at: Set(
                if status == background_tasks::TaskStatus::Processing.as_str() {
                    Some(updated_at)
                } else {
                    None
                },
            ),
            completed_at: Set(None),
            result: Set(None),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("insert process import task")
    }

    #[test]
    fn race_uploads_are_inferred_from_title_or_filename() {
        assert_eq!(
            infer_activity_type("Lumberjack 100", "2026 Lumberjack_100 Race result.gpx"),
            ActivityType::Race
        );
        assert_eq!(
            infer_activity_type("Iceman Race Day", "activity.fit"),
            ActivityType::Race
        );
        assert_eq!(
            infer_activity_type("Post Canyon Endurance", "post-canyon-endurance.gpx"),
            ActivityType::Training
        );
    }

    #[tokio::test]
    async fn manual_uploads_still_deduplicate_existing_activity() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let first = persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import first manual upload");
        assert!(matches!(first, PersistActivityUploadOutcome::Imported(_)));

        let second = persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import second manual upload");
        assert!(matches!(second, PersistActivityUploadOutcome::Duplicate(_)));

        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);
        assert_eq!(
            activity_imports::Entity::find().count(&db).await.unwrap(),
            1
        );

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[tokio::test]
    async fn stale_processing_manual_import_task_is_reset_to_pending() {
        let db = test_db().await;
        let now = Utc::now();
        let stale_at = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS + 30);
        let import = insert_processing_import(
            &db,
            Some(42),
            ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED,
            stale_at,
        )
        .await;
        insert_process_activity_import_task(
            &db,
            import.id,
            background_tasks::TaskStatus::Processing.as_str(),
            stale_at,
        )
        .await;

        let recovered = recover_stale_manual_activity_imports_for_user(
            &db,
            &TaskQueue::new(db.clone()),
            1,
            now,
        )
        .await
        .expect("recover stale import");

        assert_eq!(recovered, 1);

        let task = background_tasks::Entity::find()
            .one(&db)
            .await
            .expect("load task")
            .expect("task exists");
        assert_eq!(task.status, background_tasks::TaskStatus::Pending.as_str());
        assert_eq!(task.attempts, 0);
        assert_eq!(task.started_at, None);

        let import = activity_imports::Entity::find_by_id(import.id)
            .one(&db)
            .await
            .expect("load import")
            .expect("import exists");
        assert_eq!(import.processing_stage, ACTIVITY_IMPORT_STAGE_RAW_STORED);
        assert_eq!(import.last_processing_event_at, Some(now));
        assert_eq!(import.activity_id, Some(42));
    }

    #[tokio::test]
    async fn stale_manual_import_without_active_task_is_requeued() {
        let db = test_db().await;
        let now = Utc::now();
        let stale_at = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS + 30);
        let import =
            insert_processing_import(&db, None, ACTIVITY_IMPORT_STAGE_RAW_STORED, stale_at).await;

        let recovered = recover_stale_manual_activity_imports_for_user(
            &db,
            &TaskQueue::new(db.clone()),
            1,
            now,
        )
        .await
        .expect("recover stale import");

        assert_eq!(recovered, 1);

        let task = background_tasks::Entity::find()
            .one(&db)
            .await
            .expect("load task")
            .expect("task exists");
        assert_eq!(task.status, background_tasks::TaskStatus::Pending.as_str());
        assert!(task_targets_activity_import(&task, 1, import.id));
    }

    #[tokio::test]
    async fn fresh_processing_manual_import_task_is_left_alone() {
        let db = test_db().await;
        let now = Utc::now();
        let stale_at = now - ChronoDuration::seconds(ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS + 30);
        let import = insert_processing_import(
            &db,
            Some(42),
            ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED,
            stale_at,
        )
        .await;
        insert_process_activity_import_task(
            &db,
            import.id,
            background_tasks::TaskStatus::Processing.as_str(),
            now,
        )
        .await;

        let recovered = recover_stale_manual_activity_imports_for_user(
            &db,
            &TaskQueue::new(db.clone()),
            1,
            now,
        )
        .await
        .expect("recover stale import");

        assert_eq!(recovered, 0);

        let import = activity_imports::Entity::find_by_id(import.id)
            .one(&db)
            .await
            .expect("load import")
            .expect("import exists");
        assert_eq!(
            import.processing_stage,
            ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED
        );
    }

    #[tokio::test]
    async fn stored_activity_upload_import_waits_for_worker_processing() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let import = store_activity_upload_import(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
        )
        .await
        .expect("store raw upload");

        assert_eq!(import.status, ACTIVITY_IMPORT_STATUS_PROCESSING);
        assert_eq!(import.processing_stage, ACTIVITY_IMPORT_STAGE_RAW_STORED);
        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 0);
        assert!(std::path::Path::new(&uploads_dir)
            .join(&import.storage_path)
            .exists());

        let processed = process_stored_activity_import(
            &db,
            &uploads_dir,
            1,
            import,
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("process stored import");

        let PersistActivityUploadOutcome::Imported(imported) = processed else {
            panic!("expected stored import to create activity");
        };

        assert_eq!(imported.import.activity_id, Some(imported.activity.id));
        assert_eq!(
            imported.import.processing_stage,
            ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT
        );
        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[tokio::test]
    async fn stored_activity_upload_import_marks_duplicates_without_new_activity() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let first = match persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import first manual upload")
        {
            PersistActivityUploadOutcome::Imported(imported) => imported,
            PersistActivityUploadOutcome::Duplicate(_) => panic!("expected first import"),
        };

        let import = store_activity_upload_import(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
        )
        .await
        .expect("store duplicate raw upload");

        let processed = process_stored_activity_import(
            &db,
            &uploads_dir,
            1,
            import,
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("process duplicate stored import");

        let PersistActivityUploadOutcome::Duplicate(duplicate) = processed else {
            panic!("expected duplicate stored import");
        };

        assert_eq!(duplicate.activity.id, first.activity.id);
        let duplicate_import = duplicate.existing_import.expect("updated import");
        assert_eq!(duplicate_import.status, ACTIVITY_IMPORT_STATUS_DUPLICATE);
        assert_eq!(duplicate_import.activity_id, Some(first.activity.id));
        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);
        assert_eq!(
            activity_imports::Entity::find().count(&db).await.unwrap(),
            2
        );

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[tokio::test]
    async fn strava_uploads_deduplicate_by_source_correlation_id() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let first = persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(Some("strava-123")),
            "strava_sync",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import first Strava upload");
        assert!(matches!(first, PersistActivityUploadOutcome::Imported(_)));

        let second = persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(Some("strava-123")),
            "strava_sync",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import second Strava upload");
        assert!(matches!(second, PersistActivityUploadOutcome::Duplicate(_)));

        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);
        assert_eq!(
            activity_imports::Entity::find().count(&db).await.unwrap(),
            1
        );

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[tokio::test]
    async fn immediate_reprocess_rebuilds_training_analysis() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let imported = match persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import activity")
        {
            PersistActivityUploadOutcome::Imported(imported) => imported,
            PersistActivityUploadOutcome::Duplicate(_) => panic!("expected imported activity"),
        };

        activity_training_analyses::Entity::delete_many()
            .filter(activity_training_analyses::Column::ActivityId.eq(imported.activity.id))
            .exec(&db)
            .await
            .expect("clear activity training analysis");

        reprocess_activity_from_import(
            &db,
            &uploads_dir,
            1,
            imported.activity.clone(),
            imported.import.clone(),
            Some(&training_profile),
        )
        .await
        .expect("reprocess activity with immediate cache refresh");

        assert_eq!(
            activity_training_analyses::Entity::find()
                .filter(activity_training_analyses::Column::ActivityId.eq(imported.activity.id))
                .count(&db)
                .await
                .expect("count activity training analyses"),
            1,
        );

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[tokio::test]
    async fn deferred_reprocess_skips_immediate_training_analysis_rebuild() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let imported = match persist_activity_upload(
            &db,
            &uploads_dir,
            "test-user",
            1,
            fit_upload(None),
            "manual_upload",
            ActivityUploadDeduplication::Enabled,
            Some(&training_profile),
        )
        .await
        .expect("import activity")
        {
            PersistActivityUploadOutcome::Imported(imported) => imported,
            PersistActivityUploadOutcome::Duplicate(_) => panic!("expected imported activity"),
        };

        activity_training_analyses::Entity::delete_many()
            .filter(activity_training_analyses::Column::ActivityId.eq(imported.activity.id))
            .exec(&db)
            .await
            .expect("clear activity training analysis");

        reprocess_activity_from_import_deferred_caches(
            &db,
            &uploads_dir,
            1,
            imported.activity.clone(),
            imported.import.clone(),
            Some(&training_profile),
        )
        .await
        .expect("reprocess activity with deferred cache refresh");

        assert_eq!(
            activity_training_analyses::Entity::find()
                .filter(activity_training_analyses::Column::ActivityId.eq(imported.activity.id))
                .count(&db)
                .await
                .expect("count activity training analyses"),
            0,
        );

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }
}
