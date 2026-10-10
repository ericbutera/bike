//! Replace Bike-generated Strava TCX with verified retained sources.
use crate::activity_data::deserialize_derived_activity_data;
use crate::activity_parser::{parse_activity_artifact, ActivityParserArtifact};
use crate::activity_recording_recovery::routes_match;
use crate::entities::{activities, activity_import_artifacts, activity_imports};
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set, TransactionTrait};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};
use tokio::io::AsyncReadExt;
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryOutcome {
    ArchiveOriginal,
    RetainedProvider,
    NativeSource,
    RecentStravaRequired,
    ArchiveRequired,
    AmbiguousArchive,
}

#[derive(Serialize)]
pub struct RecoveryPlan {
    pub import_id: i32,
    pub activity_id: i32,
    pub outcome: RecoveryOutcome,
    pub reprocessing_required: bool,
    #[serde(skip)]
    import: activity_imports::Model,
    #[serde(skip)]
    activity_updated_at: DateTime<Utc>,
    #[serde(skip)]
    source: Option<RecoverySource>,
}

struct RecoverySource {
    artifact: activity_import_artifacts::Model,
    bytes: Vec<u8>,
}

/// Operator-supplied index from an owned archive's provider ID / filename mapping.
#[derive(Deserialize)]
pub struct ArchivedActivitySource {
    pub import_id: i32,
    pub provider_activity_id: i64,
    pub original_filename: String,
    pub storage_path: String,
    pub format: String,
    pub checksum_sha256: String,
    #[serde(default)]
    pub recording_label: Option<String>,
}

pub async fn register_archive_source(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    source: ArchivedActivitySource,
    apply: bool,
) -> Result<(), AppError> {
    let import = activity_imports::Entity::find_owned(db, user_id, source.import_id)
        .await?
        .ok_or_else(|| AppError::not_found("Owned import was not found"))?;
    let activity = activities::Model::find_owned(db, import.activity_id.unwrap_or(0), user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Owned activity was not found"))?;
    let id = source.provider_activity_id.to_string();
    if import.source != "strava_sync"
        || import.format != "tcx"
        || activity.activity_import_id != Some(import.id)
        || !matches!(source.format.as_str(), "fit" | "tcx" | "gpx" | "json")
        || source.checksum_sha256.len() != 64
        || !matches_provider_id(&activity, &id)
    {
        return Err(AppError::bad_request(
            "Archive source identity or format mismatch",
        ));
    }
    let derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let artifact = activity_import_artifacts::Model {
        id: 0,
        user_id,
        activity_import_id: import.id,
        artifact_kind: "original".into(),
        source_quality: if source.format == "json" {
            "strava_archive_json".into()
        } else {
            format!("{}_original", source.format)
        },
        format: source.format,
        original_filename: source.original_filename,
        storage_path: source.storage_path,
        checksum_sha256: source.checksum_sha256,
        mime_type: None,
        size_bytes: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    let verified = load_verified_source(uploads_dir, artifact, &activity).await?;
    if activity.is_bike_activity() {
        verify_archive_route(&activity, &derived, &verified)?;
    }
    if apply {
        store_archive_source(db, &activity, source.recording_label.as_deref(), verified).await?;
    }
    Ok(())
}

async fn store_archive_source(
    db: &DatabaseConnection,
    activity: &activities::Model,
    label: Option<&str>,
    verified: RecoverySource,
) -> Result<(), AppError> {
    let transaction = db.begin().await?;
    let current = activities::Model::lock_owned(&transaction, activity.id, activity.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Owned activity was not found"))?;
    if current.updated_at != activity.updated_at {
        return Err(AppError::conflict("Archive recovery activity changed"));
    }
    let mut enriched = deserialize_derived_activity_data(current.derived_data_json.as_ref());
    let previous = enriched.recording.clone();
    if let Some(label) = label {
        enriched
            .recording
            .observe("strava_archive.activity_type", label);
    }
    if enriched.recording != previous
        && !activities::Model::store_recording(
            &transaction,
            current.id,
            current.user_id,
            current.updated_at,
            &enriched,
        )
        .await?
    {
        return Err(AppError::conflict("Archive recovery activity changed"));
    }
    let existing = activity_import_artifacts::Entity::for_import(
        &transaction,
        activity.user_id,
        verified.artifact.activity_import_id,
    )
    .await?;
    if existing.iter().any(|a| {
        a.artifact_kind == "original"
            && a.checksum_sha256 == verified.artifact.checksum_sha256
            && a.storage_path == verified.artifact.storage_path
    }) {
        transaction.commit().await?;
        return Ok(());
    }
    let mut model: activity_import_artifacts::ActiveModel = verified.artifact.into();
    model.id = sea_orm::ActiveValue::NotSet;
    model.size_bytes = Set(verified.bytes.len() as i64);
    model.insert(&transaction).await?;
    transaction.commit().await?;
    Ok(())
}

fn matches_provider_id(activity: &activities::Model, id: &str) -> bool {
    activity.source_correlation_id.as_deref() == Some(id)
        || activity.source_correlation_id.as_deref() == Some(format!("strava:{id}").as_str())
}

fn verify_archive_route(
    activity: &activities::Model,
    derived: &crate::activity_data::ActivityDerivedData,
    source: &RecoverySource,
) -> Result<(), AppError> {
    let parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &source.artifact.original_filename,
        format: &source.artifact.format,
        artifact_kind: &source.artifact.artifact_kind,
        source_quality: &source.artifact.source_quality,
        bytes: &source.bytes,
    })?;
    if (parsed.draft.started_at - activity.started_at)
        .num_seconds()
        .abs()
        > 5
        || (!derived.route_points.is_empty()
            && !routes_match(
                activity.started_at,
                &derived.route_points,
                parsed.draft.started_at,
                &parsed.derived_data.route_points,
            ))
    {
        return Err(AppError::bad_request(
            "Archive GPS/time does not match the activity",
        ));
    }
    Ok(())
}

pub fn recent_activity(started_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    started_at >= now - Duration::days(30) && started_at <= now
}

pub async fn refresh_retained_recording(
    db: &DatabaseConnection,
    uploads_dir: &str,
    activity: activities::Model,
    apply: bool,
) -> Result<bool, AppError> {
    if !activity.is_bike_activity() {
        return Ok(false);
    }
    let Some(import_id) = activity.activity_import_id else {
        return Ok(false);
    };
    let artifact = activity_import_artifacts::Entity::for_import(db, activity.user_id, import_id)
        .await?
        .into_iter()
        .filter(accepted_source)
        .max_by_key(source_priority);
    let Some(artifact) = artifact else {
        return Ok(false);
    };
    let source = load_verified_source(uploads_dir, artifact, &activity).await?;
    let parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &source.artifact.original_filename,
        format: &source.artifact.format,
        artifact_kind: &source.artifact.artifact_kind,
        source_quality: &source.artifact.source_quality,
        bytes: &source.bytes,
    })?;
    let mut derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let previous = derived.recording.clone();
    derived.recording.merge(activity.recording_context());
    derived.recording.merge(parsed.derived_data.recording);
    if derived.recording == previous {
        return Ok(false);
    }
    if apply
        && !activities::Model::store_recording(
            db,
            activity.id,
            activity.user_id,
            activity.updated_at,
            &derived,
        )
        .await?
    {
        return Err(AppError::conflict(
            "Recording source changed; inspect again",
        ));
    }
    Ok(true)
}

pub async fn plan_recovery(
    db: &DatabaseConnection,
    uploads_dir: &str,
    import: activity_imports::Model,
    now: DateTime<Utc>,
) -> Result<RecoveryPlan, AppError> {
    let activity_id = import
        .activity_id
        .ok_or_else(|| AppError::bad_request("Import has no activity"))?;
    let activity = activities::Model::find_owned(db, activity_id, import.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Owned recovery activity was not found"))?;
    if activity.activity_import_id != Some(import.id) || import.source != "strava_sync" {
        return Err(AppError::bad_request(
            "Import is not the activity's Strava source",
        ));
    }
    let own = activity_import_artifacts::Entity::for_import(db, import.user_id, import.id).await?;
    let mut plan = RecoveryPlan {
        import_id: import.id,
        activity_id,
        outcome: RecoveryOutcome::ArchiveRequired,
        reprocessing_required: activity.is_bike_activity(),
        import,
        activity_updated_at: activity.updated_at,
        source: None,
    };
    if let Some(artifact) = own
        .iter()
        .find(|a| accepted_source(a) && a.storage_path == plan.import.storage_path)
    {
        load_verified_source(uploads_dir, artifact.clone(), &activity).await?;
        plan.outcome = RecoveryOutcome::NativeSource;
        return Ok(plan);
    }
    let archive_sources = matching_archive_sources(db, uploads_dir, &activity).await?;
    if archive_sources.len() > 1 {
        plan.outcome = RecoveryOutcome::AmbiguousArchive;
        return Ok(plan);
    }
    if let Some(source) = archive_sources.into_iter().next() {
        plan.outcome = RecoveryOutcome::ArchiveOriginal;
        plan.source = Some(source);
    } else if let Some(artifact) = own
        .into_iter()
        .filter(accepted_source)
        .max_by_key(source_priority)
    {
        let outcome = if artifact.artifact_kind == "original" {
            RecoveryOutcome::ArchiveOriginal
        } else {
            RecoveryOutcome::RetainedProvider
        };
        plan.source = Some(load_verified_source(uploads_dir, artifact, &activity).await?);
        plan.outcome = outcome;
    } else if recent_activity(activity.started_at, now) {
        plan.outcome = RecoveryOutcome::RecentStravaRequired;
    }
    Ok(plan)
}

fn accepted_source(artifact: &activity_import_artifacts::Model) -> bool {
    (artifact.artifact_kind == "original"
        && (matches!(artifact.format.as_str(), "fit" | "tcx" | "gpx")
            || (artifact.format == "json" && artifact.source_quality == "strava_archive_json")))
        || (artifact.artifact_kind == "provider_payload"
            && artifact.source_quality == "strava_streams"
            && artifact.format == "json")
}

fn source_priority(artifact: &activity_import_artifacts::Model) -> i32 {
    match (artifact.artifact_kind.as_str(), artifact.format.as_str()) {
        ("original", "fit") => 4,
        ("original", "tcx") => 3,
        ("original", "json") => 3,
        ("original", "gpx") => 2,
        _ => 1,
    }
}

async fn matching_archive_sources(
    db: &DatabaseConnection,
    uploads_dir: &str,
    activity: &activities::Model,
) -> Result<Vec<RecoverySource>, AppError> {
    let derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let mut sources: Vec<RecoverySource> = Vec::new();
    for import in
        activity_imports::Entity::archive_sources_for_activity(db, activity.user_id, activity.id)
            .await?
    {
        if let Some(source) =
            original_source_for_import(db, uploads_dir, activity, import.id).await?
        {
            remember_source(&mut sources, source);
        }
    }
    for candidate in activities::Model::recording_counterparts(
        db,
        activity.user_id,
        activity.id,
        activity.started_at,
    )
    .await?
    {
        if candidate.source != "archive_url_import" {
            continue;
        }
        let route = deserialize_derived_activity_data(candidate.derived_data_json.as_ref());
        if !routes_match(
            activity.started_at,
            &derived.route_points,
            candidate.started_at,
            &route.route_points,
        ) {
            continue;
        }
        let Some(original) =
            activities::Model::find_owned(db, candidate.id, activity.user_id).await?
        else {
            continue;
        };
        if !original.is_bike_activity() {
            continue;
        }
        let Some(import_id) = original.activity_import_id else {
            continue;
        };
        if let Some(source) =
            original_source_for_import(db, uploads_dir, activity, import_id).await?
        {
            remember_source(&mut sources, source);
        }
        if sources.len() == 2 && source_priority(&sources[0].artifact) == 4 {
            break;
        }
    }
    if let Some(best) = sources
        .iter()
        .map(|source| source_priority(&source.artifact))
        .max()
    {
        sources.retain(|source| source_priority(&source.artifact) == best);
    }
    Ok(sources)
}

async fn original_source_for_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    activity: &activities::Model,
    import_id: i32,
) -> Result<Option<RecoverySource>, AppError> {
    let artifacts =
        activity_import_artifacts::Entity::for_import(db, activity.user_id, import_id).await?;
    match artifacts
        .into_iter()
        .filter(|a| a.artifact_kind == "original" && accepted_source(a))
        .max_by_key(source_priority)
    {
        Some(artifact) => Ok(Some(
            load_verified_source(uploads_dir, artifact, activity).await?,
        )),
        None => Ok(None),
    }
}

fn remember_source(sources: &mut Vec<RecoverySource>, source: RecoverySource) {
    let priority = source_priority(&source.artifact);
    if let Some(existing) = sources.first() {
        if priority < source_priority(&existing.artifact) {
            return;
        }
        if priority > source_priority(&existing.artifact) {
            sources.clear();
        }
    }
    if sources.len() < 2
        && !sources
            .iter()
            .any(|existing| existing.artifact.checksum_sha256 == source.artifact.checksum_sha256)
    {
        sources.push(source);
    }
}

async fn load_verified_source(
    uploads_dir: &str,
    mut artifact: activity_import_artifacts::Model,
    activity: &activities::Model,
) -> Result<RecoverySource, AppError> {
    let relative = Path::new(&artifact.storage_path);
    if artifact.user_id != activity.user_id
        || relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(AppError::bad_request(
            "Invalid recovery source ownership or path",
        ));
    }
    const MAX_SOURCE_BYTES: u64 = 100 * 1024 * 1024;
    let file = tokio::fs::File::open(Path::new(uploads_dir).join(relative)).await?;
    let mut bytes = Vec::new();
    file.take(MAX_SOURCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > MAX_SOURCE_BYTES {
        return Err(AppError::bad_request(
            "Recovery source exceeds 100 MiB limit",
        ));
    }
    let checksum = hex::encode(Sha256::digest(&bytes));
    if !artifact.checksum_sha256.is_empty() && artifact.checksum_sha256 != checksum {
        return Err(AppError::bad_request("Recovery source checksum mismatch"));
    }
    verify_provider_identity(&artifact, &bytes, activity)?;
    if !activity.is_bike_activity()
        && artifact.activity_import_id == activity.activity_import_id.unwrap_or(0)
    {
        artifact.checksum_sha256 = checksum;
        return Ok(RecoverySource { artifact, bytes });
    }
    let parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &artifact.original_filename,
        format: &artifact.format,
        artifact_kind: &artifact.artifact_kind,
        source_quality: &artifact.source_quality,
        bytes: &bytes,
    })?;
    if artifact.activity_import_id != activity.activity_import_id.unwrap_or(0) {
        let derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
        if !routes_match(
            activity.started_at,
            &derived.route_points,
            parsed.draft.started_at,
            &parsed.derived_data.route_points,
        ) {
            return Err(AppError::bad_request(
                "Retained original does not match the recovery activity",
            ));
        }
    }
    artifact.checksum_sha256 = checksum;
    Ok(RecoverySource { artifact, bytes })
}

fn verify_provider_identity(
    artifact: &activity_import_artifacts::Model,
    bytes: &[u8],
    activity: &activities::Model,
) -> Result<(), AppError> {
    if artifact.artifact_kind == "provider_payload" {
        let payload: crate::strava_provider_payload::StoredStravaProviderPayload =
            serde_json::from_slice(bytes)
                .map_err(|_| AppError::bad_request("Invalid retained provider source"))?;
        let id = payload.activity.id.to_string();
        let correlation = activity.source_correlation_id.as_deref();
        if (correlation != Some(id.as_str())
            && correlation != Some(format!("strava:{id}").as_str()))
            || payload
                .provider_activity_id
                .is_some_and(|provider_id| provider_id != payload.activity.id)
        {
            return Err(AppError::bad_request("Retained provider identity mismatch"));
        }
    }
    Ok(())
}

pub async fn apply_recovery(
    db: &DatabaseConnection,
    uploads_dir: &str,
    mut plan: RecoveryPlan,
) -> Result<bool, AppError> {
    let Some(source) = plan.source.take() else {
        return Ok(false);
    };
    let mut artifact = source.artifact;
    let mut copied_path = None;
    if artifact.activity_import_id != plan.import_id {
        let relative = Path::new(&plan.import.storage_path)
            .parent()
            .ok_or_else(|| AppError::bad_request("Recovery import has no source directory"))?
            .join(format!("recovered_{}.{}", Uuid::new_v4(), artifact.format));
        let path = Path::new(uploads_dir).join(&relative);
        if relative.is_absolute()
            || relative
                .components()
                .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(AppError::bad_request("Invalid recovery destination"));
        }
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&path, &source.bytes).await?;
        artifact.storage_path = relative.to_string_lossy().into_owned();
        copied_path = Some(path);
    }
    let result = commit_recovery(db, &plan, artifact).await;
    // Validation failures never commit. Retain the copy after infrastructure
    // failures because an interrupted commit can have an uncertain outcome.
    if result
        .as_ref()
        .err()
        .is_some_and(|error| error.status.is_client_error())
    {
        if let Some(path) = copied_path {
            tokio::fs::remove_file(path).await?;
        }
    }
    result
}

pub async fn withhold_unrecovered_source(
    db: &DatabaseConnection,
    plan: &RecoveryPlan,
) -> Result<bool, AppError> {
    if plan.outcome != RecoveryOutcome::ArchiveRequired || plan.source.is_some() {
        return Ok(false);
    }
    let transaction = db.begin().await?;
    let current =
        activities::Model::lock_owned(&transaction, plan.activity_id, plan.import.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("Owned activity was not found"))?;
    let import =
        activity_imports::Entity::find_owned(&transaction, plan.import.user_id, plan.import_id)
            .await?;
    if current.updated_at != plan.activity_updated_at || import.as_ref() != Some(&plan.import) {
        return Err(AppError::conflict("Source recovery changed; inspect again"));
    }
    if current.format.as_deref() == Some("unavailable") {
        return Ok(false);
    }
    let changed = current.withhold_unavailable_source(&transaction).await?;
    if changed {
        crate::heatmaps::projection::Projection::invalidate_owned(
            &transaction,
            current.id,
            current.user_id,
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(changed)
}

async fn commit_recovery(
    db: &DatabaseConnection,
    plan: &RecoveryPlan,
    mut artifact: activity_import_artifacts::Model,
) -> Result<bool, AppError> {
    let transaction = db.begin().await?;
    let current =
        activities::Model::lock_owned(&transaction, plan.activity_id, plan.import.user_id)
            .await?
            .ok_or_else(|| AppError::not_found("Recovery activity no longer exists"))?;
    if current.updated_at != plan.activity_updated_at
        || current.activity_import_id != Some(plan.import_id)
    {
        return Err(AppError::bad_request(
            "Recovery activity changed; plan it again",
        ));
    }
    if artifact.activity_import_id != plan.import_id {
        let mut model: activity_import_artifacts::ActiveModel = artifact.into();
        model.id = sea_orm::ActiveValue::NotSet;
        model.activity_import_id = Set(plan.import_id);
        artifact = model.insert(&transaction).await?;
    }
    if plan.reprocessing_required {
        enqueue_recovery(&transaction, plan.activity_id).await?;
    } else {
        let mut activity: activities::ActiveModel = current.into();
        activity.original_filename = Set(Some(artifact.original_filename.clone()));
        activity.format = Set(Some(artifact.format.clone()));
        activity.update(&transaction).await?;
    }
    if !plan.import.promote_source(&transaction, &artifact).await? {
        return Err(AppError::bad_request(
            "Recovery import changed; plan it again",
        ));
    }
    if plan.reprocessing_required {
        activity_imports::Entity::mark_recovery_pending(
            &transaction,
            plan.import.user_id,
            plan.import_id,
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(true)
}

async fn enqueue_recovery(
    db: &(impl sea_orm::ConnectionTrait + sea_orm::TransactionTrait),
    activity_id: i32,
) -> Result<(), AppError> {
    let job = crate::jobs::Job::ReprocessActivityImport(crate::jobs::ReprocessActivityImportTask {
        activity_id,
    });
    crate::background_jobs::durable::Model::enqueue(
        db,
        job.task_type().to_string(),
        serde_json::to_value(job).map_err(|error| AppError::internal(error.to_string()))?,
        None,
        3,
    )
    .await?;
    Ok(())
}

/// Remove a retired generated file only after a verified replacement finished replay.
pub async fn retire_generated_artifact(
    db: &DatabaseConnection,
    uploads_dir: &str,
    artifact: activity_import_artifacts::Model,
    apply: bool,
) -> Result<bool, AppError> {
    if artifact.artifact_kind != "generated_export" || artifact.source_quality != "generated_tcx" {
        return Err(AppError::bad_request(
            "Artifact is not a retired generated file",
        ));
    }
    let Some(import) =
        activity_imports::Entity::find_owned(db, artifact.user_id, artifact.activity_import_id)
            .await?
    else {
        return Err(AppError::not_found("Owned import was not found"));
    };
    let Some(activity_id) = import.activity_id else {
        return Ok(false);
    };
    let activity = activities::Model::find_owned(db, activity_id, artifact.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Owned activity was not found"))?;
    if import.status != "processed" || activity.activity_import_id != Some(import.id) {
        return Ok(false);
    }
    let primary = activity_import_artifacts::Entity::for_import(db, import.user_id, import.id)
        .await?
        .into_iter()
        .find(|a| accepted_source(a) && a.storage_path == import.storage_path);
    let Some(primary) = primary else {
        return Ok(false);
    };
    load_verified_source(uploads_dir, primary, &activity).await?;
    let relative = Path::new(&artifact.storage_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(AppError::bad_request("Invalid generated artifact path"));
    }
    if !apply {
        return Ok(true);
    }
    let transaction = db.begin().await?;
    let current = activities::Model::lock_owned(&transaction, activity_id, artifact.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Owned activity was not found"))?;
    let current_import =
        activity_imports::Entity::find_owned(&transaction, import.user_id, import.id).await?;
    if current.updated_at != activity.updated_at || current_import.as_ref() != Some(&import) {
        return Err(AppError::conflict(
            "Recovery changed; inspect cleanup again",
        ));
    }
    if activity_imports::Entity::references_path(&transaction, &artifact.storage_path).await? {
        return Err(AppError::conflict(
            "Generated path is still an import source",
        ));
    }
    if activity_import_artifacts::Entity::shares_path(&transaction, &artifact).await? {
        return Err(AppError::conflict(
            "Generated path is shared by another artifact",
        ));
    }
    match tokio::fs::remove_file(Path::new(uploads_dir).join(relative)).await {
        Ok(()) => (),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    let model: activity_import_artifacts::ActiveModel = artifact.into();
    model.delete(&transaction).await?;
    transaction.commit().await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_refresh_has_exact_thirty_day_boundary_and_rejects_future_dates() {
        let now = DateTime::from_timestamp(1_800_000_000, 0).unwrap();
        assert!(recent_activity(now - Duration::days(30), now));
        assert!(recent_activity(now, now));
        assert!(!recent_activity(
            now - Duration::days(30) - Duration::seconds(1),
            now
        ));
        assert!(!recent_activity(now + Duration::seconds(1), now));
    }
}
