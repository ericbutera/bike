use crate::activity_details::ActivityRoutePoint;
use crate::activity_import_lock::{
    ensure_user_activity_import_lock_stage, mark_user_activity_import_lock_stage,
    release_user_activity_import_lock, ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, mark_activity_import_failed,
    mark_activity_import_processing_stage, mark_activity_imports_processed,
    reprocess_activity_from_import_deferred_caches, ReprocessedActivityImport,
    ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT, ACTIVITY_IMPORT_STAGE_RAW_STORED,
    ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT, ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT,
    ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT, ACTIVITY_IMPORT_STATUS_FAILED,
    ACTIVITY_IMPORT_STATUS_PROCESSED,
};
use crate::activity_training_analysis::rebuild_activity_training_analysis_cache;
use crate::analytics::{
    mark_segment_activity_changes, mark_user_activity_change, mark_user_fitness_dirty,
};
use crate::analytics::{rebuild_activity_analytics_cache, rebuild_segment_analytics_cache};
use crate::app_error::AppError;
use crate::dedupe::{activity_duplicate_candidate_key, activity_models_match_for_dedupe};
use crate::entities::{
    activities, activity_analytics, activity_imports, activity_training_analyses, segment_efforts,
};
use crate::segment_support::{
    clear_segment_efforts_for_activity, replace_segment_efforts_for_activity,
};
use crate::tasks::TaskQueue;
use crate::training_profile::{load_training_profile, TrainingProfile};
use chrono::{NaiveDate, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};
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
        self.affected_segment_ids
            .extend(reprocessed.affected_segment_ids);
        self.fitness_dirty_from_day = Some(match self.fitness_dirty_from_day {
            Some(current) => current.min(reprocessed.fitness_dirty_from_day),
            None => reprocessed.fitness_dirty_from_day,
        });
    }

    fn record_failure(&mut self) {
        self.failed_count += 1;
    }

    fn processed_count(&self) -> usize {
        self.succeeded_count + self.failed_count
    }

    fn dedup_ids(&mut self) {
        self.activity_ids.sort_unstable();
        self.activity_ids.dedup();
        self.affected_segment_ids.sort_unstable();
        self.affected_segment_ids.dedup();
    }
}

struct ResumedActivityImport {
    import: activity_imports::Model,
    activity_id: i32,
}

#[derive(Default)]
struct ResumeImportProgress {
    reprocessed: ActivityReprocessProgress,
    stage_ready_imports: Vec<ResumedActivityImport>,
}

impl ResumeImportProgress {
    fn record_success(
        &mut self,
        import: activity_imports::Model,
        reprocessed: ReprocessedActivityImport,
    ) {
        let activity_id = reprocessed.activity.id;
        self.reprocessed.record_success(import.id, reprocessed);
        self.stage_ready_imports.push(ResumedActivityImport {
            import,
            activity_id,
        });
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
    fn new(
        db: &'a DatabaseConnection,
        activity_import: activity_imports::Model,
        activity_id: i32,
    ) -> Self {
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

    async fn mark_raw_stored(&mut self) -> Result<(), AppError> {
        self.mark_stage(ACTIVITY_IMPORT_STAGE_RAW_STORED).await
    }

    async fn mark_segments_built(&mut self) -> Result<(), AppError> {
        self.mark_stage(ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT).await
    }

    async fn rebuild_segment_analytics(
        &mut self,
        affected_segment_ids: &[i32],
    ) -> Result<(), AppError> {
        rebuild_segment_analytics_cache(self.db, affected_segment_ids).await?;
        self.mark_stage(ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT)
            .await
    }

    async fn rebuild_activity_analytics(&mut self) -> Result<(), AppError> {
        rebuild_activity_analytics_cache(self.db, &[self.activity_id]).await?;
        self.mark_stage(ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT)
            .await
    }

    async fn rebuild_training_analysis(&mut self) -> Result<(), AppError> {
        rebuild_activity_training_analysis_cache(self.db, &[self.activity_id]).await?;
        self.mark_stage(ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT)
            .await
    }

    async fn fail_current_stage(&self, error: &AppError) -> Result<(), AppError> {
        mark_activity_import_failed(
            self.db,
            &self.activity_import,
            &self.activity_import.processing_stage,
            error,
        )
        .await
    }

    async fn mark_stage(&mut self, stage: &str) -> Result<(), AppError> {
        self.activity_import = mark_activity_import_processing_stage(
            self.db,
            &self.activity_import,
            stage,
            Some(self.activity_id),
        )
        .await?;
        Ok(())
    }
}

pub async fn refresh_activity_derived_state<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    route_points: &[ActivityRoutePoint],
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let affected_segment_ids = refresh_activity_derived_state_without_cache_rebuilds(
        db,
        user_id,
        activity_id,
        route_points,
    )
    .await?;

    rebuild_segment_analytics_cache(db, &affected_segment_ids).await?;
    rebuild_activity_analytics_cache(db, &[activity_id]).await?;

    Ok(affected_segment_ids)
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

pub async fn delete_activity_with_derived_state(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
) -> Result<Vec<i32>, AppError> {
    let mut upload_path_to_remove = None;
    let txn = db.begin().await?;
    let affected_segment_ids = load_segment_ids_for_activity(&txn, activity.id).await?;

    clear_segment_efforts_for_activity(&txn, user_id, activity.id).await?;

    activity_analytics::Entity::delete_many()
        .filter(activity_analytics::Column::ActivityId.eq(activity.id))
        .exec(&txn)
        .await?;

    activity_training_analyses::Entity::delete_many()
        .filter(activity_training_analyses::Column::ActivityId.eq(activity.id))
        .exec(&txn)
        .await?;

    activities::Entity::delete_many()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Id.eq(activity.id))
        .exec(&txn)
        .await?;

    if let Some(activity_import_id) = activity.activity_import_id {
        let remaining_references = activities::Entity::find()
            .filter(activities::Column::UserId.eq(user_id))
            .filter(activities::Column::ActivityImportId.eq(Some(activity_import_id)))
            .count(&txn)
            .await?;

        if remaining_references == 0 {
            let linked_import = activity_imports::Entity::find()
                .filter(activity_imports::Column::Id.eq(activity_import_id))
                .filter(activity_imports::Column::UserId.eq(user_id))
                .one(&txn)
                .await?;

            if let Some(linked_import) = linked_import {
                upload_path_to_remove =
                    Some(Path::new(uploads_dir).join(&linked_import.storage_path));

                activity_imports::Entity::delete_many()
                    .filter(activity_imports::Column::Id.eq(activity_import_id))
                    .filter(activity_imports::Column::UserId.eq(user_id))
                    .exec(&txn)
                    .await?;
            }
        }
    }

    txn.commit().await?;

    if let Some(upload_path) = upload_path_to_remove {
        remove_upload_file(upload_path).await;
    }

    Ok(affected_segment_ids)
}

pub async fn cleanup_duplicate_activities_for_user(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<DuplicateActivityCleanupSummary, AppError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let cleanup_plan = plan_duplicate_activity_cleanup(activities);

    if cleanup_plan.duplicate_activity_ids.is_empty() {
        return Ok(DuplicateActivityCleanupSummary {
            duplicate_group_count: 0,
            deleted_activity_count: 0,
            retained_activity_count: 0,
        });
    }

    let activities_by_id = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Id.is_in(cleanup_plan.duplicate_activity_ids.iter().copied()))
        .all(db)
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
        affected_segment_ids
            .extend(delete_activity_with_derived_state(db, uploads_dir, user_id, activity).await?);
    }

    let changed_at = Utc::now();
    if let Some(dirty_from_day) = fitness_dirty_from_day {
        mark_user_fitness_dirty(db, user_id, dirty_from_day, changed_at).await?;
    } else {
        mark_user_activity_change(db, user_id, changed_at).await?;
    }
    mark_segment_activity_changes(db, &affected_segment_ids, changed_at).await?;
    tasks.rebuild_fitness_freshness(user_id).await;
    tasks.rebuild_segment_analytics(affected_segment_ids).await;

    Ok(DuplicateActivityCleanupSummary {
        duplicate_group_count: cleanup_plan.duplicate_group_count,
        deleted_activity_count: cleanup_plan.duplicate_activity_ids.len(),
        retained_activity_count: cleanup_plan.retained_activity_ids.len(),
    })
}

pub async fn load_segment_ids_for_activity<C>(
    db: &C,
    activity_id: i32,
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait,
{
    let mut segment_ids = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::ActivityId.eq(activity_id))
        .all(db)
        .await?
        .into_iter()
        .map(|effort| effort.segment_id)
        .collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();

    Ok(segment_ids)
}

fn plan_duplicate_activity_cleanup(
    activities: Vec<activities::Model>,
) -> DuplicateActivityCleanupPlan {
    let mut activities_by_key = HashMap::<String, Vec<DuplicateActivityCandidate>>::new();

    for activity in activities {
        let route_point_count = crate::activity_details::deserialize_derived_activity_data(
            activity.derived_data_json.as_ref(),
        )
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
            if let Some((_, has_duplicates)) = keepers.iter_mut().find(|(keeper, _)| {
                activity_models_match_for_dedupe(&keeper.activity, &candidate.activity)
            }) {
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
        activity.original_filename.as_deref().and_then(|filename| {
            Path::new(filename)
                .extension()
                .and_then(|value| value.to_str())
        })
    });

    match format {
        Some(value) if value.eq_ignore_ascii_case("fit") => 3,
        Some(value) if value.eq_ignore_ascii_case("tcx") => 2,
        Some(value) if value.eq_ignore_ascii_case("gpx") => 1,
        _ => 0,
    }
}

pub async fn regenerate_segments_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Vec<i32>, AppError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let mut affected_segment_ids = Vec::new();

    for activity in activities {
        let route_points = crate::activity_details::deserialize_derived_activity_data(
            activity.derived_data_json.as_ref(),
        )
        .route_points;

        affected_segment_ids
            .extend(refresh_activity_derived_state(db, user_id, activity.id, &route_points).await?);
    }

    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    if !affected_segment_ids.is_empty() {
        mark_segment_activity_changes(db, &affected_segment_ids, Utc::now()).await?;
    }

    Ok(affected_segment_ids)
}

pub async fn process_user_segment_regeneration(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), AppError> {
    mark_user_activity_import_lock_stage(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = regenerate_segments_for_user(db, user_id).await;
    let release_result = release_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(_), Ok(())) => Ok(()),
    }
}

pub async fn reprocess_imported_activities_for_user(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(usize, usize), AppError> {
    let activities = load_imported_activities_for_user(db, user_id).await?;
    let total_activity_count = activities.len();
    let import_ids = activity_import_ids(&activities);

    if import_ids.is_empty() {
        return Ok((0, 0));
    }

    tracing::info!(
        user_id,
        total_activity_count,
        "starting user activity import reprocessing"
    );

    let imports_by_id = load_activity_imports_by_id(db, user_id, import_ids).await?;
    let training_profile = load_training_profile(db, user_id).await?;
    let mut progress = ActivityReprocessProgress::default();

    for activity in activities {
        reprocess_user_activity_import(
            db,
            uploads_dir,
            user_id,
            activity,
            &imports_by_id,
            &training_profile,
            &mut progress,
        )
        .await;
        log_user_reprocessing_progress(user_id, total_activity_count, &progress);
    }

    let reprocessed_count = progress.succeeded_count;
    let failed_count = progress.failed_count;
    finalize_user_activity_reprocessing(db, tasks, user_id, progress).await?;

    Ok((reprocessed_count, failed_count))
}

async fn load_imported_activities_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Vec<activities::Model>, AppError> {
    activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::ActivityImportId.is_not_null())
        .all(db)
        .await
        .map_err(AppError::from)
}

fn activity_import_ids(activities: &[activities::Model]) -> Vec<i32> {
    activities
        .iter()
        .filter_map(|activity| activity.activity_import_id)
        .collect()
}

async fn load_activity_imports_by_id(
    db: &DatabaseConnection,
    user_id: i32,
    import_ids: Vec<i32>,
) -> Result<HashMap<i32, activity_imports::Model>, AppError> {
    Ok(activity_imports::Entity::find()
        .filter(activity_imports::Column::UserId.eq(user_id))
        .filter(activity_imports::Column::Id.is_in(import_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|activity_import| (activity_import.id, activity_import))
        .collect())
}

async fn reprocess_user_activity_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    imports_by_id: &HashMap<i32, activity_imports::Model>,
    training_profile: &TrainingProfile,
    progress: &mut ActivityReprocessProgress,
) {
    let Some(activity_import_id) = activity.activity_import_id else {
        return;
    };
    let activity_id = activity.id;
    let Some(activity_import) = imports_by_id.get(&activity_import_id).cloned() else {
        progress.record_failure();
        tracing::warn!(
            user_id,
            activity_id,
            activity_import_id,
            "skipping activity reprocess because the linked import record is missing"
        );
        return;
    };

    match reprocess_activity_from_import_deferred_caches(
        db,
        uploads_dir,
        user_id,
        activity,
        activity_import,
        Some(training_profile),
    )
    .await
    {
        Ok(reprocessed) => progress.record_success(activity_import_id, reprocessed),
        Err(error) => {
            progress.record_failure();
            tracing::warn!(
                user_id,
                activity_id,
                error = %error.message,
                "failed to reprocess imported activity from stored source file"
            );
        }
    }
}

fn log_user_reprocessing_progress(
    user_id: i32,
    total_activity_count: usize,
    progress: &ActivityReprocessProgress,
) {
    let processed_count = progress.processed_count();
    if processed_count.is_multiple_of(25) || processed_count == total_activity_count {
        tracing::info!(
            user_id,
            processed_count,
            total_activity_count,
            reprocessed_count = progress.succeeded_count,
            failed_count = progress.failed_count,
            "user activity import reprocessing progress"
        );
    }
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
    rebuild_segment_analytics_cache(db, &progress.affected_segment_ids).await?;
    rebuild_activity_analytics_cache(db, &progress.activity_ids).await?;
    rebuild_activity_training_analysis_cache(db, &progress.activity_ids).await?;

    finalize_activity_import_batch(
        db,
        tasks,
        user_id,
        progress.affected_segment_ids,
        progress.fitness_dirty_from_day,
        Utc::now(),
    )
    .await?;
    mark_activity_imports_processed(db, &progress.import_ids).await
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

    let activities_by_import_id =
        load_activities_for_incomplete_imports(db, user_id, &imports).await?;
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
    activity_imports::Entity::find()
        .filter(activity_imports::Column::UserId.eq(user_id))
        .filter(activity_imports::Column::Status.ne(ACTIVITY_IMPORT_STATUS_PROCESSED))
        .order_by_asc(activity_imports::Column::CreatedAt)
        .all(db)
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
    let mut activities_by_import_id =
        load_activities_by_activity_import_id(db, user_id, &activity_import_ids).await?;

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
    Ok(activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(
            activities::Column::ActivityImportId.is_in(
                activity_import_ids
                    .iter()
                    .copied()
                    .map(Some)
                    .collect::<Vec<_>>(),
            ),
        )
        .all(db)
        .await?
        .into_iter()
        .filter_map(|activity| {
            activity
                .activity_import_id
                .map(|import_id| (import_id, activity))
        })
        .collect())
}

async fn load_activities_by_id(
    db: &DatabaseConnection,
    user_id: i32,
    activity_ids: &[i32],
) -> Result<Vec<activities::Model>, AppError> {
    activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Id.is_in(activity_ids.iter().copied()))
        .all(db)
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
    let Some(activity) = activities_by_import_id.get(&import.id).cloned() else {
        progress.record_failure();
        let error = AppError::internal(format!(
            "Activity import {} has no linked activity to resume",
            import.id
        ));
        mark_activity_import_failed(db, &import, &import.processing_stage, &error).await?;
        return Ok(());
    };

    match reprocess_activity_from_import_deferred_caches(
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
            let import = mark_activity_import_processing_stage(
                db,
                &import,
                ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT,
                Some(reprocessed.activity.id),
            )
            .await?;
            progress.record_success(import, reprocessed);
        }
        Err(error) => {
            progress.record_failure();
            mark_activity_import_failed(db, &import, &import.processing_stage, &error).await?;
        }
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
    mut progress: ResumeImportProgress,
) -> Result<(), AppError> {
    if progress.succeeded_count() == 0 {
        return Ok(());
    }

    progress.reprocessed.dedup_ids();
    rebuild_segment_analytics_cache(db, &progress.reprocessed.affected_segment_ids).await?;
    let stage_ready_imports = mark_resumed_imports_processing_stage(
        db,
        progress.stage_ready_imports,
        ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT,
    )
    .await?;

    rebuild_activity_analytics_cache(db, &progress.reprocessed.activity_ids).await?;
    let stage_ready_imports = mark_resumed_imports_processing_stage(
        db,
        stage_ready_imports,
        ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT,
    )
    .await?;

    rebuild_activity_training_analysis_cache(db, &progress.reprocessed.activity_ids).await?;
    mark_resumed_imports_processing_stage(
        db,
        stage_ready_imports,
        ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT,
    )
    .await?;

    finalize_activity_import_batch(
        db,
        tasks,
        user_id,
        progress.reprocessed.affected_segment_ids,
        progress.reprocessed.fitness_dirty_from_day,
        Utc::now(),
    )
    .await?;
    mark_activity_imports_processed(db, &progress.reprocessed.import_ids).await
}

async fn mark_resumed_imports_processing_stage(
    db: &DatabaseConnection,
    imports: Vec<ResumedActivityImport>,
    stage: &str,
) -> Result<Vec<ResumedActivityImport>, AppError> {
    let mut marked = Vec::new();

    for resumed in imports {
        let import = mark_activity_import_processing_stage(
            db,
            &resumed.import,
            stage,
            Some(resumed.activity_id),
        )
        .await?;
        marked.push(ResumedActivityImport {
            import,
            activity_id: resumed.activity_id,
        });
    }

    Ok(marked)
}

pub async fn process_single_activity_import_reprocessing(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    activity_id: i32,
) -> Result<ActivityImportReprocessSummary, AppError> {
    let activity = load_activity_for_reprocessing(db, activity_id).await?;
    let user_id = activity.user_id;

    ensure_user_activity_import_lock_stage(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = run_single_activity_import_reprocessing(db, uploads_dir, tasks, activity).await;

    let release_result = release_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
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

    let reprocessed = reprocess_single_activity_from_import(
        &mut context,
        uploads_dir,
        user_id,
        activity,
        &training_profile,
    )
    .await?;
    context.set_activity_id(reprocessed.activity.id);
    let mut affected_segment_ids = reprocessed.affected_segment_ids;
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    rebuild_single_activity_reprocessing_caches(&mut context, &affected_segment_ids).await?;

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
    activities::Entity::find_by_id(activity_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found(format!("Activity {activity_id} was not found")))
}

fn activity_import_id_for_reprocessing(activity: &activities::Model) -> Result<i32, AppError> {
    activity.activity_import_id.ok_or_else(|| {
        AppError::bad_request(format!(
            "Activity {} is not linked to a stored import",
            activity.id
        ))
    })
}

async fn load_activity_import_for_reprocessing(
    db: &DatabaseConnection,
    activity_import_id: i32,
) -> Result<activity_imports::Model, AppError> {
    activity_imports::Entity::find_by_id(activity_import_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            AppError::not_found(format!(
                "Activity import {activity_import_id} was not found"
            ))
        })
}

async fn reprocess_single_activity_from_import(
    context: &mut SingleActivityReprocessContext<'_>,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    training_profile: &TrainingProfile,
) -> Result<ReprocessedActivityImport, AppError> {
    context.mark_raw_stored().await?;

    match reprocess_activity_from_import_deferred_caches(
        context.db,
        uploads_dir,
        user_id,
        activity,
        context.activity_import.clone(),
        Some(training_profile),
    )
    .await
    {
        Ok(reprocessed) => Ok(reprocessed),
        Err(error) => {
            context.fail_current_stage(&error).await?;
            Err(error)
        }
    }
}

async fn rebuild_single_activity_reprocessing_caches(
    context: &mut SingleActivityReprocessContext<'_>,
    affected_segment_ids: &[i32],
) -> Result<(), AppError> {
    context.mark_segments_built().await?;
    context
        .rebuild_segment_analytics(affected_segment_ids)
        .await?;
    context.rebuild_activity_analytics().await?;
    context.rebuild_training_analysis().await
}

async fn finalize_single_activity_reprocessing(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    finalization: SingleActivityReprocessFinalization,
) -> Result<ActivityImportReprocessSummary, AppError> {
    let affected_segment_count = finalization.affected_segment_ids.len();

    finalize_activity_import_batch(
        db,
        tasks,
        finalization.user_id,
        finalization.affected_segment_ids,
        Some(finalization.fitness_dirty_from_day),
        Utc::now(),
    )
    .await?;
    mark_activity_imports_processed(db, &[finalization.activity_import_id]).await?;

    Ok(ActivityImportReprocessSummary {
        activity_id: finalization.activity_id,
        activity_import_id: finalization.activity_import_id,
        affected_segment_count,
    })
}

pub async fn process_user_activity_import_reprocessing(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(), AppError> {
    ensure_user_activity_import_lock_stage(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = reprocess_imported_activities_for_user(db, uploads_dir, tasks, user_id).await;
    let release_result = release_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok((reprocessed_count, failed_count)), Ok(())) => {
            tracing::info!(
                user_id,
                reprocessed_count,
                failed_count,
                "completed user activity import reprocessing"
            );
            Ok(())
        }
    }
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
            sport: "ride".to_string(),
            source: "manual_upload".to_string(),
            source_correlation_id: None,
            original_filename: original_filename.map(str::to_string),
            format: format.map(str::to_string),
            activity_type: crate::activity_type::ActivityType::Training
                .as_str()
                .to_string(),
            started_at: DateTime::parse_from_rfc3339("2026-05-11T13:23:17Z")
                .expect("started_at")
                .with_timezone(&Utc),
            ended_at: None,
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
            estimated_ftp_watts: None,
            heart_rate_zones_json: None,
            derived_data_json: Some(
                serialize_derived_activity_data(&ActivityDerivedData {
                    laps: Vec::new(),
                    chart_points: Vec::new(),
                    route_points,
                })
                .expect("serialize derived data"),
            ),
            created_at,
            updated_at: created_at,
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

        let first_activity = make_activity(
            20,
            created_at,
            Some("fit"),
            Some("first.fit"),
            northern_route,
        );
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
