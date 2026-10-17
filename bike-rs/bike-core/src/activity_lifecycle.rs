use crate::activity_details::ActivityRoutePoint;
use crate::activity_import_lock::{
    ensure_user_activity_import_lock_stage, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::process_stored_activity_import;
use crate::activity_import_pipeline::reprocess_activity_from_import;
use crate::activity_import_pipeline::ActivityUploadDeduplication;
use crate::activity_import_pipeline::PersistActivityUploadOutcome;
use crate::activity_import_pipeline::ReprocessedActivityImport;
use crate::activity_import_pipeline::ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT;
use crate::activity_import_pipeline::ACTIVITY_IMPORT_STATUS_FAILED;
use crate::analytics::{mark_segment_activity_changes, mark_user_activity_change, mark_user_fitness_dirty};
use crate::dedupe::{activity_duplicate_candidate_key, activity_models_match_for_dedupe};
use crate::entities::{activities, activity_analytics, activity_imports, activity_training_analyses, segment_efforts};
use crate::jobs::JobQueue as TaskQueue;
use crate::segment_support::{clear_segment_efforts_for_activity, replace_segment_efforts_for_activity};
use crate::training_profile::{load_training_profile, TrainingProfile};
use crate::workflow_error::WorkflowError as AppError;
use chrono::{NaiveDate, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, TransactionSession, TransactionTrait};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateActivityCleanupSummary {
    pub duplicate_group_count: usize,
    pub deleted_activity_count: usize,
    pub retained_activity_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityImportReprocessSummary {
    pub activity_id: i32,
    pub activity_import_id: i32,
    pub affected_segment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DuplicateActivityCleanupPlan {
    duplicate_group_count: usize,
    duplicate_activity_ids: Vec<i32>,
    retained_activity_ids: Vec<i32>,
}

#[derive(Debug, Clone)]
struct DuplicateActivityCandidate {
    activity: activities::Model,
    route_point_count: usize,
    format_rank: i32,
}

struct SingleActivityReprocessFinalization {
    user_id: i32,
    activity_id: i32,
    activity_import_id: i32,
    affected_segment_ids: Vec<i32>,
    fitness_dirty_from_day: NaiveDate,
}

#[derive(Default)]
struct ActivityReprocessProgress {
    succeeded_count: usize,
    failed_count: usize,
    affected_segment_ids: Vec<i32>,
    activity_ids: Vec<i32>,
    import_ids: Vec<i32>,
    fitness_dirty_from_day: Option<NaiveDate>,
}

impl ActivityReprocessProgress {
    fn record_success(&mut self, import_id: i32, reprocessed: ReprocessedActivityImport) {
        self.succeeded_count += 1;
        self.activity_ids.push(reprocessed.activity.id);
        self.import_ids.push(import_id);
        self.affected_segment_ids.extend(reprocessed.affected_segment_ids);
        self.fitness_dirty_from_day = Some(match self.fitness_dirty_from_day {
            Some(current) => current.min(reprocessed.fitness_dirty_from_day),
            None => reprocessed.fitness_dirty_from_day,
        });
    }

    fn record_failure(&mut self) {
        self.failed_count += 1;
    }

    fn dedup_ids(&mut self) {
        self.activity_ids.sort_unstable();
        self.activity_ids.dedup();
        self.affected_segment_ids.sort_unstable();
        self.affected_segment_ids.dedup();
    }
}

#[derive(Default)]
struct ResumeImportProgress {
    reprocessed: ActivityReprocessProgress,
}

impl ResumeImportProgress {
    fn record_success(&mut self, import: activity_imports::Model, reprocessed: ReprocessedActivityImport) {
        self.reprocessed.record_success(import.id, reprocessed);
    }

    fn record_failure(&mut self) {
        self.reprocessed.record_failure();
    }

    fn succeeded_count(&self) -> usize {
        self.reprocessed.succeeded_count
    }

    fn failed_count(&self) -> usize {
        self.reprocessed.failed_count
    }
}

struct SingleActivityReprocessContext<'a> {
    db: &'a DatabaseConnection,
    activity_import: activity_imports::Model,
    activity_id: i32,
}

impl<'a> SingleActivityReprocessContext<'a> {
    fn new(db: &'a DatabaseConnection, activity_import: activity_imports::Model, activity_id: i32) -> Self {
        Self {
            db,
            activity_import,
            activity_id,
        }
    }

    fn activity_import_id(&self) -> i32 {
        self.activity_import.id
    }

    fn activity_id(&self) -> i32 {
        self.activity_id
    }

    fn set_activity_id(&mut self, activity_id: i32) {
        self.activity_id = activity_id;
    }
}

pub async fn refresh_activity_derived_state_without_cache_rebuilds<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    route_points: &[ActivityRoutePoint],
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut affected_segment_ids = load_segment_ids_for_activity(db, activity_id).await?;

    replace_segment_efforts_for_activity(db, user_id, activity_id, route_points).await?;

    affected_segment_ids.extend(load_segment_ids_for_activity(db, activity_id).await?);
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    Ok(affected_segment_ids)
}

pub async fn delete_activity_with_derived_state<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
) -> Result<Vec<i32>, AppError> {
    let txn = db.begin().await?;
    let deleted = delete_activity_records(&txn, user_id, activity).await?;
    txn.commit().await?;
    if let Some(source) = deleted.source {
        remove_upload_file(Path::new(uploads_dir).join(source)).await;
    }
    Ok(deleted.segment_ids)
}

pub struct DeletedActivity {
    pub segment_ids: Vec<i32>,
    pub source: Option<String>,
}

#[tracing::instrument(name = "activity.delete.records", skip_all, fields(user_id, activity_id = activity.id))]
pub async fn delete_activity_records(
    db: &impl ConnectionTrait,
    user_id: i32,
    activity: activities::Model,
) -> Result<DeletedActivity, AppError> {
    let mut source = None;
    let affected_segment_ids = load_segment_ids_for_activity(db, activity.id).await?;

    clear_segment_efforts_for_activity(db, user_id, activity.id).await?;

    activity_analytics::Entity::delete_by_activity_id(db, activity.id).await?;
    activity_training_analyses::Entity::delete_by_activity_id(db, activity.id).await?;
    activities::Entity::delete_by_user(db, activity.id, user_id).await?;

    if let Some(activity_import_id) = activity.activity_import_id {
        let remaining_references =
            activities::Entity::count_by_import_for_user(db, activity_import_id, user_id).await?;

        if remaining_references == 0 {
            let linked_import = activity_imports::Entity::find_owned(db, user_id, activity_import_id).await?;

            if let Some(linked_import) = linked_import {
                source = Some(linked_import.storage_path);

                activity_imports::Entity::delete_by_user(db, activity_import_id, user_id).await?;
            }
        }
    }

    Ok(DeletedActivity {
        segment_ids: affected_segment_ids,
        source,
    })
}

pub async fn cleanup_duplicate_activities_for_user(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<DuplicateActivityCleanupSummary, AppError> {
    let activities = activities::Model::for_user(db, user_id).await?;
    let cleanup_plan = plan_duplicate_activity_cleanup(activities);

    if cleanup_plan.duplicate_activity_ids.is_empty() {
        return Ok(DuplicateActivityCleanupSummary {
            duplicate_group_count: 0,
            deleted_activity_count: 0,
            retained_activity_count: 0,
        });
    }

    let activities_by_id =
        activities::Model::list_owned_by_ids(db, user_id, cleanup_plan.duplicate_activity_ids.iter().copied())
            .await?
            .into_iter()
            .map(|activity| (activity.id, activity))
            .collect::<HashMap<_, _>>();
    let mut affected_segment_ids = Vec::new();
    let mut fitness_dirty_from_day: Option<chrono::NaiveDate> = None;

    for activity_id in &cleanup_plan.duplicate_activity_ids {
        let Some(activity) = activities_by_id.get(activity_id).cloned() else {
            tracing::warn!(
                user_id,
                activity_id,
                "skipping duplicate cleanup because the activity was already removed"
            );
            continue;
        };

        fitness_dirty_from_day = Some(match fitness_dirty_from_day {
            Some(current) => current.min(activity.started_at.date_naive()),
            None => activity.started_at.date_naive(),
        });
        affected_segment_ids.extend(delete_activity_with_derived_state(db, uploads_dir, user_id, activity).await?);
    }

    let changed_at = Utc::now();
    if let Some(dirty_from_day) = fitness_dirty_from_day {
        mark_user_fitness_dirty(db, user_id, dirty_from_day, changed_at).await?;
    } else {
        mark_user_activity_change(db, user_id, changed_at).await?;
    }
    mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    tasks.rebuild_fitness_freshness(user_id).await?;
    tasks.rebuild_segment_analytics(affected_segment_ids).await?;

    Ok(DuplicateActivityCleanupSummary {
        duplicate_group_count: cleanup_plan.duplicate_group_count,
        deleted_activity_count: cleanup_plan.duplicate_activity_ids.len(),
        retained_activity_count: cleanup_plan.retained_activity_ids.len(),
    })
}

pub async fn load_segment_ids_for_activity<C>(db: &C, activity_id: i32) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait,
{
    let mut segment_ids = segment_efforts::Model::segment_ids_for_activity(db, activity_id).await?;
    segment_ids.sort_unstable();
    segment_ids.dedup();

    Ok(segment_ids)
}

fn plan_duplicate_activity_cleanup(activities: Vec<activities::Model>) -> DuplicateActivityCleanupPlan {
    let mut activities_by_key = HashMap::<String, Vec<DuplicateActivityCandidate>>::new();

    for activity in activities {
        let route_point_count =
            crate::activity_details::deserialize_derived_activity_data(activity.derived_data_json.as_ref())
                .route_points
                .len();
        let key = activity_duplicate_candidate_key(activity.started_at, &activity.sport);

        activities_by_key
            .entry(key)
            .or_default()
            .push(DuplicateActivityCandidate {
                format_rank: activity_cleanup_format_rank(&activity),
                route_point_count,
                activity,
            });
    }

    let mut grouped_candidates = activities_by_key.into_iter().collect::<Vec<_>>();
    grouped_candidates.sort_by(|left, right| left.0.cmp(&right.0));

    let mut duplicate_activity_ids = Vec::new();
    let mut retained_activity_ids = Vec::new();
    let mut duplicate_group_count = 0usize;

    for (_, mut candidates) in grouped_candidates {
        if candidates.len() < 2 {
            continue;
        }

        candidates.sort_by(compare_duplicate_activity_candidates);

        let mut keepers = Vec::<(DuplicateActivityCandidate, bool)>::new();

        for candidate in candidates {
            if let Some((_, has_duplicates)) = keepers
                .iter_mut()
                .find(|(keeper, _)| activity_models_match_for_dedupe(&keeper.activity, &candidate.activity))
            {
                *has_duplicates = true;
                duplicate_activity_ids.push(candidate.activity.id);
            } else {
                keepers.push((candidate, false));
            }
        }

        for (keeper, has_duplicates) in keepers {
            if has_duplicates {
                duplicate_group_count += 1;
                retained_activity_ids.push(keeper.activity.id);
            }
        }
    }

    duplicate_activity_ids.sort_unstable();
    retained_activity_ids.sort_unstable();

    DuplicateActivityCleanupPlan {
        duplicate_group_count,
        duplicate_activity_ids,
        retained_activity_ids,
    }
}

fn compare_duplicate_activity_candidates(
    left: &DuplicateActivityCandidate,
    right: &DuplicateActivityCandidate,
) -> Ordering {
    right
        .format_rank
        .cmp(&left.format_rank)
        .then_with(|| right.route_point_count.cmp(&left.route_point_count))
        .then_with(|| left.activity.created_at.cmp(&right.activity.created_at))
        .then_with(|| left.activity.id.cmp(&right.activity.id))
}

fn activity_cleanup_format_rank(activity: &activities::Model) -> i32 {
    let format = activity.format.as_deref().or_else(|| {
        activity
            .original_filename
            .as_deref()
            .and_then(|filename| Path::new(filename).extension().and_then(|value| value.to_str()))
    });

    match format {
        Some(value) if value.eq_ignore_ascii_case("fit") => 3,
        Some(value) if value.eq_ignore_ascii_case("tcx") => 2,
        Some(value) if value.eq_ignore_ascii_case("gpx") => 1,
        _ => 0,
    }
}

pub async fn regenerate_segments_for_user(db: &DatabaseConnection, user_id: i32) -> Result<Vec<i32>, AppError> {
    crate::segment_regeneration::regenerate_segments_for_user(db, user_id)
        .await
        .map_err(|error| AppError::internal(error.message))
}

pub async fn process_user_segment_regeneration(db: &DatabaseConnection, user_id: i32) -> Result<(), AppError> {
    crate::segment_regeneration::process_user_segment_regeneration(db, user_id)
        .await
        .map_err(|error| AppError::internal(error.message))
}

async fn finalize_user_activity_reprocessing(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    mut progress: ActivityReprocessProgress,
) -> Result<(), AppError> {
    if progress.succeeded_count == 0 {
        return Ok(());
    }

    progress.dedup_ids();

    crate::activity_import_lifecycle::complete_activity_imports(
        db,
        tasks,
        user_id,
        &progress.import_ids,
        progress.affected_segment_ids,
        progress.fitness_dirty_from_day,
    )
    .await
    .map_err(AppError::from)
}

pub async fn resume_incomplete_activity_imports_for_user(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(usize, usize), AppError> {
    let imports = load_incomplete_activity_imports_for_user(db, user_id).await?;

    if imports.is_empty() {
        return Ok((0, 0));
    }

    tracing::info!(
        user_id,
        import_count = imports.len(),
        "resuming incomplete activity imports"
    );

    let activities_by_import_id = load_activities_for_incomplete_imports(db, user_id, &imports).await?;
    let training_profile = load_training_profile(db, user_id).await?;
    let mut progress = ResumeImportProgress::default();

    for import in imports {
        resume_activity_import(
            db,
            uploads_dir,
            user_id,
            import,
            &activities_by_import_id,
            &training_profile,
            &mut progress,
        )
        .await?;
    }

    let resumed_count = progress.succeeded_count();
    let failed_count = progress.failed_count();
    finalize_resumed_activity_imports(db, tasks, user_id, progress).await?;

    Ok((resumed_count, failed_count))
}

async fn load_incomplete_activity_imports_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Vec<activity_imports::Model>, AppError> {
    activity_imports::Entity::resumable_for_user(db, user_id)
        .await
        .map_err(AppError::from)
}

async fn load_activities_for_incomplete_imports(
    db: &DatabaseConnection,
    user_id: i32,
    imports: &[activity_imports::Model],
) -> Result<HashMap<i32, activities::Model>, AppError> {
    let activity_import_ids = imports.iter().map(|import| import.id).collect::<Vec<_>>();
    let activity_ids = imports
        .iter()
        .filter_map(|import| import.activity_id)
        .collect::<Vec<_>>();
    let mut activities_by_import_id = load_activities_by_activity_import_id(db, user_id, &activity_import_ids).await?;

    if !activity_ids.is_empty() {
        for activity in load_activities_by_id(db, user_id, &activity_ids).await? {
            if let Some(import_id) = activity.activity_import_id {
                activities_by_import_id.insert(import_id, activity);
            }
        }
    }

    Ok(activities_by_import_id)
}

async fn load_activities_by_activity_import_id(
    db: &DatabaseConnection,
    user_id: i32,
    activity_import_ids: &[i32],
) -> Result<HashMap<i32, activities::Model>, AppError> {
    Ok(
        activities::Model::linked_by_import_ids(db, user_id, activity_import_ids)
            .await?
            .into_iter()
            .filter_map(|activity| activity.activity_import_id.map(|import_id| (import_id, activity)))
            .collect(),
    )
}

async fn load_activities_by_id(
    db: &DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<Vec<activities::Model>, AppError> {
    activities::Model::list_owned_by_ids(db, user_id, activity_ids.iter().copied())
        .await
        .map_err(AppError::from)
}

async fn resume_activity_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    import: activity_imports::Model,
    activities_by_import_id: &HashMap<i32, activities::Model>,
    training_profile: &TrainingProfile,
    progress: &mut ResumeImportProgress,
) -> Result<(), AppError> {
    log_failed_import_resume_retry(user_id, &import);
    if crate::entities::activity_import_attempts::Entity::active(db, user_id, import.id)
        .await?
        .is_some()
    {
        return Ok(());
    }
    let Some(activity) = activities_by_import_id.get(&import.id).cloned() else {
        match crate::activity_import_pipeline::plan_activity_import_replay(db, uploads_dir, &import, "raw_stored").await
        {
            Ok(_) => {}
            Err(error) if matches!(error.status.as_u16(), 400 | 404) => return Ok(()),
            Err(error) => return Err(error),
        }
        match process_stored_activity_import(
            db,
            uploads_dir,
            user_id,
            import.clone(),
            ActivityUploadDeduplication::Enabled,
            Some(training_profile),
        )
        .await
        {
            Ok(PersistActivityUploadOutcome::Imported(persisted)) => progress.record_success(
                persisted.import,
                ReprocessedActivityImport {
                    activity: persisted.activity,
                    affected_segment_ids: persisted.affected_segment_ids,
                    fitness_dirty_from_day: persisted.fitness_dirty_from_day,
                },
            ),
            Ok(PersistActivityUploadOutcome::Duplicate(_)) => {}
            Err(_) => progress.record_failure(),
        }
        return Ok(());
    };

    match reprocess_activity_from_import(
        db,
        uploads_dir,
        user_id,
        activity,
        import.clone(),
        Some(training_profile),
    )
    .await
    {
        Ok(reprocessed) => {
            progress.record_success(import, reprocessed);
        }
        Err(_) => progress.record_failure(),
    }

    Ok(())
}

fn log_failed_import_resume_retry(user_id: i32, import: &activity_imports::Model) {
    if import.status == ACTIVITY_IMPORT_STATUS_FAILED
        && import.processing_stage == ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT
    {
        tracing::info!(
            user_id,
            import_id = import.id,
            "retrying failed import from completed derived checkpoint"
        );
    }
}

async fn finalize_resumed_activity_imports(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    progress: ResumeImportProgress,
) -> Result<(), AppError> {
    finalize_user_activity_reprocessing(db, tasks, user_id, progress.reprocessed).await
}

pub async fn process_single_activity_import_reprocessing(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    activity_id: i32,
) -> Result<ActivityImportReprocessSummary, AppError> {
    let activity = load_activity_for_reprocessing(db, activity_id).await?;
    if crate::background_jobs::batches::current().is_some() {
        return run_single_activity_import_reprocessing(db, uploads_dir, tasks, activity).await;
    }
    let user_id = activity.user_id;

    ensure_user_activity_import_lock_stage(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = run_single_activity_import_reprocessing(db, uploads_dir, tasks, activity).await;

    let release_result =
        release_user_activity_import_lock(db, user_id, ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING).await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(summary), Ok(())) => Ok(summary),
    }
}

async fn run_single_activity_import_reprocessing(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    activity: activities::Model,
) -> Result<ActivityImportReprocessSummary, AppError> {
    let user_id = activity.user_id;
    let activity_import_id = activity_import_id_for_reprocessing(&activity)?;
    let activity_import = load_activity_import_for_reprocessing(db, activity_import_id).await?;
    let training_profile = load_training_profile(db, user_id).await?;
    let mut context = SingleActivityReprocessContext::new(db, activity_import, activity.id);

    let reprocessed =
        reprocess_single_activity_from_import(&mut context, uploads_dir, user_id, activity, &training_profile).await?;
    context.set_activity_id(reprocessed.activity.id);
    let mut affected_segment_ids = reprocessed.affected_segment_ids;
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    finalize_single_activity_reprocessing(
        db,
        tasks,
        SingleActivityReprocessFinalization {
            user_id,
            activity_id: context.activity_id(),
            activity_import_id: context.activity_import_id(),
            affected_segment_ids,
            fitness_dirty_from_day: reprocessed.fitness_dirty_from_day,
        },
    )
    .await
}

async fn load_activity_for_reprocessing(
    db: &DatabaseConnection,
    activity_id: i32,
) -> Result<activities::Model, AppError> {
    let activity = activities::Model::find_by_id(db, activity_id)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Activity {activity_id} was not found")))?;
    if activity.source == "strava_sync" {
        return Err(AppError::bad_request("Use Strava sync or backfill for Strava imports"));
    }
    Ok(activity)
}

fn activity_import_id_for_reprocessing(activity: &activities::Model) -> Result<i32, AppError> {
    activity
        .activity_import_id
        .ok_or_else(|| AppError::bad_request(format!("Activity {} is not linked to a stored import", activity.id)))
}

async fn load_activity_import_for_reprocessing(
    db: &DatabaseConnection,
    activity_import_id: i32,
) -> Result<activity_imports::Model, AppError> {
    activity_imports::Entity::find_for_administration(db, activity_import_id)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Activity import {activity_import_id} was not found")))
}

async fn reprocess_single_activity_from_import(
    context: &mut SingleActivityReprocessContext<'_>,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    training_profile: &TrainingProfile,
) -> Result<ReprocessedActivityImport, AppError> {
    reprocess_activity_from_import(
        context.db,
        uploads_dir,
        user_id,
        activity,
        context.activity_import.clone(),
        Some(training_profile),
    )
    .await
}

async fn finalize_single_activity_reprocessing(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    finalization: SingleActivityReprocessFinalization,
) -> Result<ActivityImportReprocessSummary, AppError> {
    let affected_segment_count = finalization.affected_segment_ids.len();

    crate::activity_import_lifecycle::complete_activity_imports(
        db,
        tasks,
        finalization.user_id,
        &[finalization.activity_import_id],
        finalization.affected_segment_ids,
        Some(finalization.fitness_dirty_from_day),
    )
    .await?;

    Ok(ActivityImportReprocessSummary {
        activity_id: finalization.activity_id,
        activity_import_id: finalization.activity_import_id,
        affected_segment_count,
    })
}

async fn remove_upload_file(path: PathBuf) {
    if let Err(error) = tokio::fs::remove_file(&path).await {
        if error.kind() != ErrorKind::NotFound {
            tracing::warn!(
                error = ?error,
                path = %path.display(),
                "failed to remove upload file for deleted activity"
            );
        }
    }
}

#[cfg(test)]
mod tests {

    mod deletion;

    use super::*;
    use crate::activity_details::{serialize_derived_activity_data, ActivityDerivedData};
    use chrono::{DateTime, Duration};

    fn make_route_point(elapsed_seconds: i32, latitude: f64, longitude: f64) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds,
            latitude,
            longitude,
            distance_meters: Some(elapsed_seconds as f64 * 10.0),
            elevation_meters: Some(100.0),
            speed_mps: Some(8.0),
            heart_rate_bpm: Some(140),
            cadence_rpm: Some(88),
            power_watts: None,
        }
    }

    fn make_activity(
        id: i32,
        created_at: chrono::DateTime<Utc>,
        format: Option<&str>,
        original_filename: Option<&str>,
        route_points: Vec<ActivityRoutePoint>,
    ) -> activities::Model {
        activities::Model {
            id,
            user_id: 42,
            activity_import_id: Some(id + 1000),
            title: format!("Activity {id}"),
            original_filename: original_filename.map(str::to_string),
            format: format.map(str::to_string),
            started_at: DateTime::parse_from_rfc3339("2026-05-11T13:23:17Z")
                .expect("started_at")
                .with_timezone(&Utc),
            distance_meters: Some(122_768.0),
            moving_time_seconds: Some(27_011),
            total_time_seconds: Some(27_012),
            elevation_gain_meters: Some(500.0),
            elevation_loss_meters: Some(500.0),
            average_speed_mps: Some(7.5),
            max_speed_mps: Some(14.0),
            average_heart_rate_bpm: Some(140),
            max_heart_rate_bpm: Some(170),
            average_cadence_rpm: Some(86),
            max_cadence_rpm: Some(102),
            calories: Some(900),
            derived_data_json: Some(
                serialize_derived_activity_data(&ActivityDerivedData {
                    recording: Default::default(),
                    laps: Vec::new(),
                    chart_points: Vec::new(),
                    route_points,
                })
                .expect("serialize derived data"),
            ),
            created_at,
            updated_at: created_at,
            ..crate::test_support::activity()
        }
    }

    #[test]
    fn plans_duplicate_cleanup_prefers_fit_over_tcx() {
        let created_at = Utc::now();
        let route = vec![
            make_route_point(0, 44.7500, -85.6200),
            make_route_point(1800, 44.7200, -85.4200),
            make_route_point(3600, 44.7500, -85.6200),
        ];
        let fit_activity = make_activity(
            10,
            created_at + Duration::minutes(5),
            Some("fit"),
            Some("ride.fit"),
            route.clone(),
        );
        let tcx_activity = make_activity(11, created_at, Some("tcx"), Some("ride.tcx"), route);

        let plan = plan_duplicate_activity_cleanup(vec![fit_activity, tcx_activity]);

        assert_eq!(plan.duplicate_group_count, 1);
        assert_eq!(plan.retained_activity_ids, vec![10]);
        assert_eq!(plan.duplicate_activity_ids, vec![11]);
    }

    #[test]
    fn plans_duplicate_cleanup_skips_same_base_key_with_different_routes() {
        let created_at = Utc::now();
        let northern_route = vec![
            make_route_point(0, 45.1000, -122.1000),
            make_route_point(1800, 45.1200, -122.1200),
            make_route_point(3600, 45.1400, -122.1400),
        ];
        let southern_route = vec![
            make_route_point(0, 40.1000, -120.1000),
            make_route_point(1800, 40.1200, -120.1200),
            make_route_point(3600, 40.1400, -120.1400),
        ];

        let first_activity = make_activity(20, created_at, Some("fit"), Some("first.fit"), northern_route);
        let second_activity = make_activity(
            21,
            created_at + Duration::minutes(2),
            Some("fit"),
            Some("second.fit"),
            southern_route,
        );

        let plan = plan_duplicate_activity_cleanup(vec![first_activity, second_activity]);

        assert_eq!(plan.duplicate_group_count, 0);
        assert!(plan.retained_activity_ids.is_empty());
        assert!(plan.duplicate_activity_ids.is_empty());
    }

    #[test]
    fn plans_duplicate_cleanup_for_trimmed_tcx_variant() {
        let created_at = Utc::now();
        let full_route = vec![
            make_route_point(0, 44.7539, -85.6290),
            make_route_point(1800, 44.7600, -85.6000),
            make_route_point(3600, 44.7420, -85.5109),
        ];
        let trimmed_route = vec![
            make_route_point(295, 44.7552, -85.6176),
            make_route_point(1800, 44.7600, -85.6000),
            make_route_point(3300, 44.7414, -85.5091),
        ];

        let mut fit_activity = make_activity(
            1039,
            created_at + Duration::minutes(5),
            Some("fit"),
            Some("22742351729_ACTIVITY.fit"),
            full_route,
        );
        fit_activity.distance_meters = Some(28_396.91);
        fit_activity.moving_time_seconds = Some(11_107);
        fit_activity.total_time_seconds = Some(12_923);

        let mut tcx_activity = make_activity(
            1129,
            created_at,
            Some("tcx"),
            Some("MSB_cleanup_18351119858.tcx"),
            trimmed_route,
        );
        tcx_activity.distance_meters = Some(28_396.90);
        tcx_activity.moving_time_seconds = Some(12_925);
        tcx_activity.total_time_seconds = Some(12_925);

        let plan = plan_duplicate_activity_cleanup(vec![fit_activity, tcx_activity]);

        assert_eq!(plan.duplicate_group_count, 1);
        assert_eq!(plan.retained_activity_ids, vec![1039]);
        assert_eq!(plan.duplicate_activity_ids, vec![1129]);
    }
}
