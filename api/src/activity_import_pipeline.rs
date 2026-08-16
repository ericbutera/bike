#![deny(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use crate::activity_details::serialize_derived_activity_data;
use crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds;
use crate::activity_parser::{parse_activity_artifact, ActivityParserArtifact, ParsedActivityData};
use crate::activity_training_analysis::rebuild_activity_training_analysis_cache;
use crate::activity_type::ActivityType;
use crate::analytics::{
    mark_segment_activity_changes, mark_user_activity_change, mark_user_fitness_dirty,
    rebuild_activity_analytics_cache, rebuild_segment_analytics_cache,
};
use crate::app_error::AppError;
use crate::dedupe::activity_dedupe_matches_model;
use crate::entities::{activities, activity_import_artifacts, activity_imports};
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
use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
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

pub struct PersistActivityUploadRequest<'a> {
    pub uploads_dir: &'a str,
    pub user_storage_key: &'a str,
    pub user_id: i32,
    pub upload: ActivityUploadPayload,
    pub source: &'a str,
    pub deduplication: ActivityUploadDeduplication,
    pub training_profile: Option<&'a TrainingProfile>,
}

pub struct PersistActivityUploadWithArtifactsRequest<'a> {
    pub uploads_dir: &'a str,
    pub user_storage_key: &'a str,
    pub user_id: i32,
    pub upload: ActivityUploadPayload,
    pub primary_artifact_kind: &'a str,
    pub primary_source_quality: &'a str,
    pub additional_artifacts: Vec<ActivityImportArtifactPayload>,
    pub source: &'a str,
    pub deduplication: ActivityUploadDeduplication,
    pub training_profile: Option<&'a TrainingProfile>,
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
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_PARSED: &str = "activity_parsed";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED: &str = "activity_saved";
pub const ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT: &str = "segments_built";
pub const ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT: &str = "segment_analytics_built";
pub const ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT: &str = "activity_analytics_built";
pub const ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT: &str = "training_analysis_built";
pub const ACTIVITY_IMPORT_STAGE_COMPLETE: &str = "complete";
pub const ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS: i64 = 300;
pub const ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL: &str = "original";
pub const ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD: &str = "provider_payload";
pub const ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT: &str = "generated_export";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL: &str = "fit_original";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_TCX_ORIGINAL: &str = "tcx_original";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_GPX_ORIGINAL: &str = "gpx_original";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS: &str = "strava_streams";
pub const ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX: &str = "generated_tcx";
pub const ACTIVITY_IMPORT_VERSION_LEGACY: i32 = activity_imports::ACTIVITY_IMPORT_VERSION_LEGACY;
pub const ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE: i32 =
    activity_imports::ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE;
pub const ACTIVITY_IMPORT_VERSION_CURRENT: i32 = activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT;
pub const ACTIVITY_PROCESSING_PROVIDER: &str = "activity_processing";
const PROCESS_ACTIVITY_IMPORT_TASK_TYPE: &str = "process_activity_import";
const MANUAL_UPLOAD_SOURCE: &str = "manual_upload";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ActivityProcessingNode {
    RawStored,
    ActivityParsed,
    ActivitySaved,
    SegmentsBuilt,
    SegmentAnalyticsBuilt,
    ActivityAnalyticsBuilt,
    TrainingAnalysisBuilt,
}

impl ActivityProcessingNode {
    pub fn id(self) -> &'static str {
        match self {
            Self::RawStored => "raw_stored",
            Self::ActivityParsed => "activity_parsed",
            Self::ActivitySaved => "activity_saved",
            Self::SegmentsBuilt => "segments_built",
            Self::SegmentAnalyticsBuilt => "segment_analytics_built",
            Self::ActivityAnalyticsBuilt => "activity_analytics_built",
            Self::TrainingAnalysisBuilt => "training_analysis_built",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::RawStored => "Raw stored",
            Self::ActivityParsed => "Activity parsed",
            Self::ActivitySaved => "Activity saved",
            Self::SegmentsBuilt => "Segments built",
            Self::SegmentAnalyticsBuilt => "Segment analytics built",
            Self::ActivityAnalyticsBuilt => "Activity analytics built",
            Self::TrainingAnalysisBuilt => "Training analysis built",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivityProcessingGraphNode {
    pub node: ActivityProcessingNode,
    pub stage: &'static str,
    pub depends_on: &'static [ActivityProcessingNode],
}

const NO_DEPENDENCIES: &[ActivityProcessingNode] = &[];
const PARSE_DEPENDENCIES: &[ActivityProcessingNode] = &[ActivityProcessingNode::RawStored];
const ACTIVITY_SAVE_DEPENDENCIES: &[ActivityProcessingNode] =
    &[ActivityProcessingNode::ActivityParsed];
const SEGMENT_DEPENDENCIES: &[ActivityProcessingNode] = &[ActivityProcessingNode::ActivitySaved];
const SEGMENT_ANALYTICS_DEPENDENCIES: &[ActivityProcessingNode] =
    &[ActivityProcessingNode::SegmentsBuilt];
const ACTIVITY_ANALYTICS_DEPENDENCIES: &[ActivityProcessingNode] =
    &[ActivityProcessingNode::SegmentAnalyticsBuilt];
const TRAINING_ANALYSIS_DEPENDENCIES: &[ActivityProcessingNode] =
    &[ActivityProcessingNode::ActivityAnalyticsBuilt];

const ACTIVITY_PROCESSING_GRAPH_NODES: &[ActivityProcessingGraphNode] = &[
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::RawStored,
        stage: ACTIVITY_IMPORT_STAGE_RAW_STORED,
        depends_on: NO_DEPENDENCIES,
    },
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::ActivityParsed,
        stage: ACTIVITY_IMPORT_STAGE_ACTIVITY_PARSED,
        depends_on: PARSE_DEPENDENCIES,
    },
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::ActivitySaved,
        stage: ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED,
        depends_on: ACTIVITY_SAVE_DEPENDENCIES,
    },
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::SegmentsBuilt,
        stage: ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT,
        depends_on: SEGMENT_DEPENDENCIES,
    },
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::SegmentAnalyticsBuilt,
        stage: ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT,
        depends_on: SEGMENT_ANALYTICS_DEPENDENCIES,
    },
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::ActivityAnalyticsBuilt,
        stage: ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT,
        depends_on: ACTIVITY_ANALYTICS_DEPENDENCIES,
    },
    ActivityProcessingGraphNode {
        node: ActivityProcessingNode::TrainingAnalysisBuilt,
        stage: ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT,
        depends_on: TRAINING_ANALYSIS_DEPENDENCIES,
    },
];

pub fn activity_processing_graph_nodes() -> &'static [ActivityProcessingGraphNode] {
    ACTIVITY_PROCESSING_GRAPH_NODES
}

pub fn activity_processing_graph_mermaid() -> String {
    let mut lines = vec!["flowchart LR".to_string()];
    for graph_node in ACTIVITY_PROCESSING_GRAPH_NODES {
        lines.push(format!(
            "  {}[\"{}\"]",
            graph_node.node.id(),
            graph_node.node.label()
        ));
    }
    for graph_node in ACTIVITY_PROCESSING_GRAPH_NODES {
        for dependency in graph_node.depends_on {
            lines.push(format!(
                "  {} --> {}",
                dependency.id(),
                graph_node.node.id()
            ));
        }
    }
    lines.join("\n")
}

pub fn activity_processing_graph() -> DiGraphMap<ActivityProcessingNode, ()> {
    let mut graph = DiGraphMap::new();
    for graph_node in ACTIVITY_PROCESSING_GRAPH_NODES {
        graph.add_node(graph_node.node);
        for dependency in graph_node.depends_on {
            graph.add_edge(*dependency, graph_node.node, ());
        }
    }
    graph
}

pub fn activity_processing_topological_order() -> Result<Vec<ActivityProcessingNode>, AppError> {
    toposort(&activity_processing_graph(), None).map_err(|cycle| {
        AppError::internal(format!(
            "Activity processing graph contains a cycle at {:?}",
            cycle.node_id()
        ))
    })
}

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

struct ActivityProcessingRun<'a> {
    db: &'a DatabaseConnection,
    uploads_dir: &'a str,
    user_id: i32,
    import: activity_imports::Model,
    existing_activity: Option<activities::Model>,
    source_correlation_id: Option<String>,
    deduplication: ActivityUploadDeduplication,
    training_profile: Option<&'a TrainingProfile>,
    cache_refresh: ReprocessCacheRefresh,
}

struct ActivityProcessingState {
    import_model: activity_imports::Model,
    activity_model: Option<activities::Model>,
    parsed_activity: Option<ParsedActivityData>,
    affected_segment_ids: Vec<i32>,
    parsing_artifact: ActivityProcessingArtifact,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct ActivityProcessingArtifact {
    artifact_kind: String,
    original_filename: String,
    format: String,
    source_quality: String,
    storage_path: String,
}

struct ActivitySaveData {
    activity_type: ActivityType,
    estimated_ftp_watts: Option<i32>,
    heart_rate_zones_json: Option<crate::training_profile::StoredActivityHeartRateZones>,
    derived_data_json: crate::activity_details::StoredActivityDerivedData,
}

struct ReprocessActivityImportRequest<'a> {
    db: &'a DatabaseConnection,
    uploads_dir: &'a str,
    user_id: i32,
    activity: activities::Model,
    activity_import: activity_imports::Model,
    training_profile: Option<&'a TrainingProfile>,
    cache_refresh: ReprocessCacheRefresh,
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

async fn run_activity_processing_graph(
    run: ActivityProcessingRun<'_>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let graph_nodes_by_node = activity_processing_graph_nodes()
        .iter()
        .map(|node| (node.node, *node))
        .collect::<HashMap<_, _>>();
    let mut state = load_activity_processing_state(&run).await?;

    for node in activity_processing_topological_order()? {
        let graph_node = graph_nodes_by_node.get(&node).ok_or_else(|| {
            AppError::internal(format!(
                "Activity processing graph is missing metadata for {node:?}"
            ))
        })?;

        if let Some(outcome) = run_activity_processing_node(&run, &mut state, *graph_node).await? {
            return Ok(outcome);
        }
        if should_stop_activity_processing(&run, node) {
            break;
        }
    }

    activity_processing_result(state)
}

async fn load_activity_processing_state(
    run: &ActivityProcessingRun<'_>,
) -> Result<ActivityProcessingState, AppError> {
    let parsing_artifact = load_best_activity_parsing_artifact(run.db, &run.import).await?;
    let bytes =
        tokio::fs::read(Path::new(run.uploads_dir).join(&parsing_artifact.storage_path)).await?;
    Ok(ActivityProcessingState {
        import_model: run.import.clone(),
        activity_model: run.existing_activity.clone(),
        parsed_activity: None,
        affected_segment_ids: Vec::new(),
        parsing_artifact,
        bytes,
    })
}

async fn load_best_activity_parsing_artifact(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
) -> Result<ActivityProcessingArtifact, AppError> {
    let artifacts = activity_import_artifacts::Entity::find()
        .filter(activity_import_artifacts::Column::ActivityImportId.eq(import.id))
        .filter(activity_import_artifacts::Column::UserId.eq(import.user_id))
        .all(db)
        .await?;

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

fn activity_processing_artifact_from_model(
    artifact: activity_import_artifacts::Model,
) -> ActivityProcessingArtifact {
    ActivityProcessingArtifact {
        artifact_kind: artifact.artifact_kind,
        original_filename: artifact.original_filename,
        format: artifact.format,
        source_quality: artifact.source_quality,
        storage_path: artifact.storage_path,
    }
}

fn legacy_activity_processing_artifact(
    import: &activity_imports::Model,
) -> Option<ActivityProcessingArtifact> {
    if !activity_format_is_parsable(&import.format) {
        return None;
    }

    Some(ActivityProcessingArtifact {
        artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL.to_string(),
        original_filename: import.original_filename.clone(),
        format: import.format.clone(),
        source_quality: original_source_quality_for_format(&import.format).to_string(),
        storage_path: import.storage_path.clone(),
    })
}

fn activity_artifact_is_parsable(artifact: &activity_import_artifacts::Model) -> bool {
    match artifact.artifact_kind.as_str() {
        ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL | ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT => {
            activity_format_is_parsable(&artifact.format)
        }
        ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD => {
            artifact.source_quality == ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS
                && artifact.format == "json"
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
        (
            ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
            ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
            _,
        ) => 500,
        (
            ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
            ACTIVITY_IMPORT_SOURCE_QUALITY_TCX_ORIGINAL,
            _,
        ) => 400,
        (
            ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
            ACTIVITY_IMPORT_SOURCE_QUALITY_GPX_ORIGINAL,
            _,
        ) => 300,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, _, "fit") => 490,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, _, "tcx") => 390,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, _, "gpx") => 290,
        (
            ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD,
            ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS,
            "json",
        ) => 200,
        (
            ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
            ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
            "tcx",
        ) => 100,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT, _, "tcx") => 90,
        (ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT, _, "gpx") => 80,
        _ => 0,
    }
}

async fn run_activity_processing_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    graph_node: ActivityProcessingGraphNode,
) -> Result<Option<PersistActivityUploadOutcome>, AppError> {
    match graph_node.node {
        ActivityProcessingNode::RawStored => mark_raw_stored_node(run, state).await?,
        ActivityProcessingNode::ActivityParsed => {
            parse_activity_node(run, state, graph_node.stage).await?;
            if let Some(outcome) = prevent_duplicate_activity(run, state).await? {
                return Ok(Some(outcome));
            }
        }
        ActivityProcessingNode::ActivitySaved => {
            save_activity_node(run, state, graph_node.stage).await?
        }
        ActivityProcessingNode::SegmentsBuilt => {
            build_segments_node(run, state, graph_node.stage).await?
        }
        ActivityProcessingNode::SegmentAnalyticsBuilt => {
            build_segment_analytics_node(run, state, graph_node.stage).await?;
        }
        ActivityProcessingNode::ActivityAnalyticsBuilt => {
            build_activity_analytics_node(run, state, graph_node.stage).await?;
        }
        ActivityProcessingNode::TrainingAnalysisBuilt => {
            build_training_analysis_node(run, state, graph_node.stage).await?;
        }
    }

    Ok(None)
}

fn should_stop_activity_processing(
    run: &ActivityProcessingRun<'_>,
    node: ActivityProcessingNode,
) -> bool {
    run.cache_refresh == ReprocessCacheRefresh::Deferred
        && node == ActivityProcessingNode::SegmentsBuilt
}

fn activity_processing_result(
    state: ActivityProcessingState,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let parsed = state.parsed_activity.ok_or_else(|| {
        AppError::internal("Activity processing graph did not run the parser node")
    })?;
    let activity = state
        .activity_model
        .ok_or_else(|| AppError::internal("Activity processing graph did not save an activity"))?;

    Ok(PersistActivityUploadOutcome::Imported(
        PersistedActivityImport {
            import: state.import_model,
            activity,
            affected_segment_ids: state.affected_segment_ids,
            fitness_dirty_from_day: parsed.draft.started_at.date_naive(),
        },
    ))
}

async fn mark_raw_stored_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
) -> Result<(), AppError> {
    if state.import_model.processing_stage == ACTIVITY_IMPORT_STAGE_RAW_STORED {
        return Ok(());
    }

    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        ACTIVITY_IMPORT_STAGE_RAW_STORED,
        state.activity_model.as_ref().map(|activity| activity.id),
    )
    .await?;
    Ok(())
}

async fn parse_activity_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    state.parsed_activity = Some(parse_activity_artifact(ActivityParserArtifact {
        original_filename: &state.parsing_artifact.original_filename,
        format: &state.parsing_artifact.format,
        artifact_kind: &state.parsing_artifact.artifact_kind,
        source_quality: &state.parsing_artifact.source_quality,
        bytes: &state.bytes,
    })?);
    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        state.activity_model.as_ref().map(|activity| activity.id),
    )
    .await?;
    Ok(())
}

async fn prevent_duplicate_activity(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
) -> Result<Option<PersistActivityUploadOutcome>, AppError> {
    if state.activity_model.is_some() || run.deduplication == ActivityUploadDeduplication::Disabled
    {
        return Ok(None);
    }

    let parsed = parsed_activity_ref(state)?;
    let Some(duplicate) =
        find_duplicate_activity(run.db, run.user_id, &parsed.draft, &parsed.derived_data).await?
    else {
        return Ok(None);
    };

    let import_model =
        mark_activity_import_duplicate(run.db, &state.import_model, duplicate.activity.id).await?;
    Ok(Some(PersistActivityUploadOutcome::Duplicate(
        DeduplicatedActivityImport {
            activity: duplicate.activity,
            existing_import: Some(import_model),
        },
    )))
}

async fn save_activity_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let parsed = parsed_activity_ref(state)?.clone();
    let training_profile = load_processing_training_profile(run).await?;
    let save_data = build_activity_save_data(
        &parsed,
        &state.parsing_artifact.original_filename,
        &training_profile,
    )?;
    let saved_activity = match state.activity_model.take() {
        Some(activity) => {
            update_activity_from_parsed(
                run.db,
                activity,
                &state.parsing_artifact,
                &parsed,
                save_data,
            )
            .await?
        }
        None => {
            insert_activity_from_parsed(
                run,
                &state.import_model,
                &state.parsing_artifact,
                &parsed,
                save_data,
            )
            .await?
        }
    };

    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        Some(saved_activity.id),
    )
    .await?;
    state.activity_model = Some(saved_activity);
    Ok(())
}

async fn load_processing_training_profile(
    run: &ActivityProcessingRun<'_>,
) -> Result<TrainingProfile, AppError> {
    match run.training_profile {
        Some(profile) => Ok(profile.clone()),
        None => load_training_profile(run.db, run.user_id).await,
    }
}

fn build_activity_save_data(
    parsed: &ParsedActivityData,
    original_filename: &str,
    training_profile: &TrainingProfile,
) -> Result<ActivitySaveData, AppError> {
    let heart_rate_zones = summarize_heart_rate_zones(
        &parsed.derived_data.route_points,
        &parsed.derived_data.chart_points,
        parsed
            .draft
            .moving_time_seconds
            .or(parsed.draft.total_time_seconds),
        parsed.draft.average_heart_rate_bpm,
        training_profile.heart_rate_zone_bounds_bpm.as_deref(),
    );

    Ok(ActivitySaveData {
        activity_type: infer_activity_type(&parsed.draft.title, original_filename),
        estimated_ftp_watts: training_profile.estimated_ftp_watts,
        heart_rate_zones_json: serialize_activity_heart_rate_zones(&heart_rate_zones)?,
        derived_data_json: serialize_derived_activity_data(&parsed.derived_data)?,
    })
}

async fn update_activity_from_parsed(
    db: &DatabaseConnection,
    activity: activities::Model,
    parsing_artifact: &ActivityProcessingArtifact,
    parsed: &ParsedActivityData,
    save_data: ActivitySaveData,
) -> Result<activities::Model, AppError> {
    let mut active_model: activities::ActiveModel = activity.into();
    apply_common_activity_fields(&mut active_model, parsing_artifact, parsed, save_data);
    active_model.update(db).await.map_err(AppError::from)
}

async fn insert_activity_from_parsed(
    run: &ActivityProcessingRun<'_>,
    import: &activity_imports::Model,
    parsing_artifact: &ActivityProcessingArtifact,
    parsed: &ParsedActivityData,
    save_data: ActivitySaveData,
) -> Result<activities::Model, AppError> {
    let mut active_model = activities::ActiveModel {
        user_id: Set(run.user_id),
        activity_import_id: Set(Some(import.id)),
        source: Set(import.source.clone()),
        source_correlation_id: Set(run.source_correlation_id.clone()),
        ..Default::default()
    };
    apply_common_activity_fields(&mut active_model, parsing_artifact, parsed, save_data);
    active_model.insert(run.db).await.map_err(AppError::from)
}

fn apply_common_activity_fields(
    active_model: &mut activities::ActiveModel,
    parsing_artifact: &ActivityProcessingArtifact,
    parsed: &ParsedActivityData,
    save_data: ActivitySaveData,
) {
    active_model.title = Set(parsed.draft.title.clone());
    active_model.sport = Set(parsed.draft.sport.clone());
    active_model.original_filename = Set(Some(parsing_artifact.original_filename.clone()));
    active_model.format = Set(Some(parsing_artifact.format.clone()));
    active_model.activity_type = Set(save_data.activity_type.as_str().to_string());
    active_model.started_at = Set(parsed.draft.started_at);
    active_model.ended_at = Set(parsed.draft.ended_at);
    active_model.distance_meters = Set(parsed.draft.distance_meters);
    active_model.moving_time_seconds = Set(parsed.draft.moving_time_seconds);
    active_model.total_time_seconds = Set(parsed.draft.total_time_seconds);
    active_model.elevation_gain_meters = Set(parsed.draft.elevation_gain_meters);
    active_model.elevation_loss_meters = Set(parsed.draft.elevation_loss_meters);
    active_model.average_speed_mps = Set(parsed.draft.average_speed_mps);
    active_model.max_speed_mps = Set(parsed.draft.max_speed_mps);
    active_model.average_heart_rate_bpm = Set(parsed.draft.average_heart_rate_bpm);
    active_model.max_heart_rate_bpm = Set(parsed.draft.max_heart_rate_bpm);
    active_model.average_cadence_rpm = Set(parsed.draft.average_cadence_rpm);
    active_model.max_cadence_rpm = Set(parsed.draft.max_cadence_rpm);
    active_model.calories = Set(parsed.draft.calories);
    active_model.estimated_ftp_watts = Set(save_data.estimated_ftp_watts);
    active_model.heart_rate_zones_json = Set(save_data.heart_rate_zones_json);
    active_model.derived_data_json = Set(Some(save_data.derived_data_json));
}

async fn build_segments_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let parsed = parsed_activity_ref(state)?;
    let activity_id = activity_model_ref(state)?.id;
    state.affected_segment_ids = refresh_activity_derived_state_without_cache_rebuilds(
        run.db,
        run.user_id,
        activity_id,
        &parsed.derived_data.route_points,
    )
    .await?;
    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        Some(activity_id),
    )
    .await?;
    Ok(())
}

async fn build_segment_analytics_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let activity_id = activity_model_ref(state)?.id;
    rebuild_segment_analytics_cache(run.db, &state.affected_segment_ids).await?;
    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        Some(activity_id),
    )
    .await?;
    Ok(())
}

async fn build_activity_analytics_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let activity_id = activity_model_ref(state)?.id;
    rebuild_activity_analytics_cache(run.db, &[activity_id]).await?;
    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        Some(activity_id),
    )
    .await?;
    Ok(())
}

async fn build_training_analysis_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let activity_id = activity_model_ref(state)?.id;
    rebuild_activity_training_analysis_cache(run.db, &[activity_id]).await?;
    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        Some(activity_id),
    )
    .await?;
    Ok(())
}

fn parsed_activity_ref(state: &ActivityProcessingState) -> Result<&ParsedActivityData, AppError> {
    state.parsed_activity.as_ref().ok_or_else(|| {
        AppError::internal("Activity processing reached a dependent node before parse")
    })
}

fn activity_model_ref(state: &ActivityProcessingState) -> Result<&activities::Model, AppError> {
    state.activity_model.as_ref().ok_or_else(|| {
        AppError::internal("Activity processing reached a dependent node before activity save")
    })
}

pub async fn persist_activity_upload(
    db: &DatabaseConnection,
    request: PersistActivityUploadRequest<'_>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let source_quality = original_source_quality_for_format(&request.upload.format);
    persist_activity_upload_with_artifacts(
        db,
        PersistActivityUploadWithArtifactsRequest {
            uploads_dir: request.uploads_dir,
            user_storage_key: request.user_storage_key,
            user_id: request.user_id,
            upload: request.upload,
            primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
            primary_source_quality: source_quality,
            additional_artifacts: Vec::new(),
            source: request.source,
            deduplication: request.deduplication,
            training_profile: request.training_profile,
        },
    )
    .await
}

pub async fn persist_activity_upload_with_artifacts(
    db: &DatabaseConnection,
    request: PersistActivityUploadWithArtifactsRequest<'_>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let source_correlation_id = request.upload.source_correlation_id.clone();

    if request.deduplication == ActivityUploadDeduplication::Enabled {
        if let Some(existing) = find_existing_activity_by_source_correlation(
            db,
            request.user_id,
            request.source,
            source_correlation_id.as_deref(),
        )
        .await?
        {
            return Ok(PersistActivityUploadOutcome::Duplicate(existing));
        }
    }

    let import_model = store_activity_upload_import_with_artifacts(
        db,
        request.uploads_dir,
        request.user_storage_key,
        request.user_id,
        request.upload,
        request.primary_artifact_kind,
        request.primary_source_quality,
        request.additional_artifacts,
        request.source,
    )
    .await?;

    let result = run_activity_processing_graph(ActivityProcessingRun {
        db,
        uploads_dir: request.uploads_dir,
        user_id: request.user_id,
        import: import_model.clone(),
        existing_activity: None,
        source_correlation_id,
        deduplication: request.deduplication,
        training_profile: request.training_profile,
        cache_refresh: ReprocessCacheRefresh::Immediate,
    })
    .await;

    if let Err(error) = &result {
        let latest_import = activity_imports::Entity::find_by_id(import_model.id)
            .one(db)
            .await?
            .unwrap_or(import_model);
        mark_activity_import_failed(db, &latest_import, &latest_import.processing_stage, error)
            .await?;
    }

    result
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
        uploads_dir,
        user_storage_key,
        user_id,
        upload,
        ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
        source_quality,
        Vec::new(),
        source,
    )
    .await
}

#[expect(
    clippy::too_many_arguments,
    reason = "shared persistence bridge keeps legacy import row and artifact rows consistent"
)]
pub async fn store_activity_upload_import_with_artifacts(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_storage_key: &str,
    user_id: i32,
    upload: ActivityUploadPayload,
    primary_artifact_kind: &str,
    primary_source_quality: &str,
    additional_artifacts: Vec<ActivityImportArtifactPayload>,
    source: &str,
) -> Result<activity_imports::Model, AppError> {
    let relative_path = activity_import_storage_path(user_storage_key, &upload.format, Utc::now());
    let full_path = Path::new(uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &upload.bytes).await?;
    let primary_artifact = ActivityImportArtifactPayload {
        artifact_kind: primary_artifact_kind.to_string(),
        format: upload.format.clone(),
        source_quality: primary_source_quality.to_string(),
        original_filename: upload.original_filename.clone(),
        mime_type: upload.mime_type.clone(),
        bytes: upload.bytes.clone(),
    };

    let import_model = activity_imports::ActiveModel {
        user_id: Set(user_id),
        import_version: Set(ACTIVITY_IMPORT_VERSION_CURRENT),
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
        storage_path: Set(relative_path.clone()),
        size_bytes: Set(upload.bytes.len() as i64),
        mime_type: Set(upload.mime_type),
        ..Default::default()
    }
    .insert(db)
    .await?;

    insert_activity_import_artifact(
        db,
        &import_model,
        primary_artifact,
        relative_path,
        upload.bytes.len() as i64,
    )
    .await?;

    for artifact in additional_artifacts {
        store_additional_activity_import_artifact(
            db,
            uploads_dir,
            user_storage_key,
            &import_model,
            artifact,
        )
        .await?;
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

pub fn original_source_quality_for_format(format: &str) -> &'static str {
    match format {
        "fit" => ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
        "tcx" => ACTIVITY_IMPORT_SOURCE_QUALITY_TCX_ORIGINAL,
        "gpx" => ACTIVITY_IMPORT_SOURCE_QUALITY_GPX_ORIGINAL,
        _ => "unknown_original",
    }
}

async fn store_additional_activity_import_artifact(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_storage_key: &str,
    import: &activity_imports::Model,
    artifact: ActivityImportArtifactPayload,
) -> Result<activity_import_artifacts::Model, AppError> {
    let relative_path =
        activity_import_storage_path(user_storage_key, &artifact.format, Utc::now());
    let full_path = Path::new(uploads_dir).join(&relative_path);

    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(&full_path, &artifact.bytes).await?;
    let size_bytes = artifact.bytes.len() as i64;
    insert_activity_import_artifact(db, import, artifact, relative_path, size_bytes).await
}

async fn insert_activity_import_artifact(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    artifact: ActivityImportArtifactPayload,
    storage_path: String,
    size_bytes: i64,
) -> Result<activity_import_artifacts::Model, AppError> {
    let checksum_sha256 = checksum_sha256_hex(&artifact.bytes);

    activity_import_artifacts::ActiveModel {
        activity_import_id: Set(import.id),
        user_id: Set(import.user_id),
        artifact_kind: Set(artifact.artifact_kind),
        format: Set(artifact.format),
        source_quality: Set(artifact.source_quality),
        original_filename: Set(artifact.original_filename),
        storage_path: Set(storage_path),
        size_bytes: Set(size_bytes),
        mime_type: Set(artifact.mime_type),
        checksum_sha256: Set(checksum_sha256),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(AppError::from)
}

fn checksum_sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub async fn process_stored_activity_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    import: activity_imports::Model,
    deduplication: ActivityUploadDeduplication,
    training_profile: Option<&TrainingProfile>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    run_activity_processing_graph(ActivityProcessingRun {
        db,
        uploads_dir,
        user_id,
        import,
        existing_activity: None,
        source_correlation_id: None,
        deduplication,
        training_profile,
        cache_refresh: ReprocessCacheRefresh::Immediate,
    })
    .await
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
    request: ReprocessActivityImportRequest<'_>,
) -> Result<ReprocessedActivityImport, AppError> {
    let outcome = run_activity_processing_graph(ActivityProcessingRun {
        db: request.db,
        uploads_dir: request.uploads_dir,
        user_id: request.user_id,
        import: request.activity_import,
        existing_activity: Some(request.activity),
        source_correlation_id: None,
        deduplication: ActivityUploadDeduplication::Disabled,
        training_profile: request.training_profile,
        cache_refresh: request.cache_refresh,
    })
    .await?;

    match outcome {
        PersistActivityUploadOutcome::Imported(imported) => Ok(ReprocessedActivityImport {
            activity: imported.activity,
            affected_segment_ids: imported.affected_segment_ids,
            fitness_dirty_from_day: imported.fitness_dirty_from_day,
        }),
        PersistActivityUploadOutcome::Duplicate(_) => Err(AppError::internal(
            "Reprocessing an existing activity cannot produce a duplicate outcome",
        )),
    }
}

pub async fn reprocess_activity_from_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    activity_import: activity_imports::Model,
    training_profile: Option<&TrainingProfile>,
) -> Result<ReprocessedActivityImport, AppError> {
    reprocess_activity_from_import_with_cache_refresh(ReprocessActivityImportRequest {
        db,
        uploads_dir,
        user_id,
        activity,
        activity_import,
        training_profile,
        cache_refresh: ReprocessCacheRefresh::Immediate,
    })
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
    reprocess_activity_from_import_with_cache_refresh(ReprocessActivityImportRequest {
        db,
        uploads_dir,
        user_id,
        activity,
        activity_import,
        training_profile,
        cache_refresh: ReprocessCacheRefresh::Deferred,
    })
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
        activities, activity_analytics, activity_import_artifacts, activity_imports,
        activity_training_analyses, integration_events, segment_efforts, segments,
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
        db.execute(&schema.create_table_from_entity(activity_import_artifacts::Entity))
            .await
            .expect("create activity import artifacts table");
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
        db.execute(&schema.create_table_from_entity(integration_events::Entity))
            .await
            .expect("create integration events table");

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

    async fn persist_test_activity_upload(
        db: &DatabaseConnection,
        uploads_dir: &str,
        upload: ActivityUploadPayload,
        source: &str,
        training_profile: &TrainingProfile,
    ) -> Result<PersistActivityUploadOutcome, AppError> {
        persist_activity_upload(
            db,
            PersistActivityUploadRequest {
                uploads_dir,
                user_storage_key: "test-user",
                user_id: 1,
                upload,
                source,
                deduplication: ActivityUploadDeduplication::Enabled,
                training_profile: Some(training_profile),
            },
        )
        .await
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

    #[test]
    fn activity_processing_graph_orders_required_dependencies() {
        let order = activity_processing_topological_order().expect("activity graph is a DAG");
        let position = order
            .iter()
            .enumerate()
            .map(|(index, node)| (*node, index))
            .collect::<std::collections::HashMap<_, _>>();

        for node in activity_processing_graph_nodes() {
            for dependency in node.depends_on {
                assert!(
                    position[dependency] < position[&node.node],
                    "{dependency:?} should run before {:?}",
                    node.node
                );
            }
        }
    }

    #[test]
    fn activity_processing_graph_mermaid_matches_graph_edges() {
        let mermaid = activity_processing_graph_mermaid();

        assert!(mermaid.starts_with("flowchart LR"));
        assert!(mermaid.contains("raw_stored[\"Raw stored\"]"));
        assert!(mermaid.contains("raw_stored --> activity_parsed"));
        assert!(mermaid.contains("activity_analytics_built --> training_analysis_built"));
    }

    #[tokio::test]
    async fn manual_uploads_still_deduplicate_existing_activity() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let first = persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(None),
            "manual_upload",
            &training_profile,
        )
        .await
        .expect("import first manual upload");
        assert!(matches!(first, PersistActivityUploadOutcome::Imported(_)));

        let second = persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(None),
            "manual_upload",
            &training_profile,
        )
        .await
        .expect("import second manual upload");
        assert!(matches!(second, PersistActivityUploadOutcome::Duplicate(_)));

        assert_eq!(activities::Entity::find().count(&db).await.unwrap(), 1);
        assert_eq!(
            activity_imports::Entity::find().count(&db).await.unwrap(),
            2
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
        let artifact = activity_import_artifacts::Entity::find()
            .filter(activity_import_artifacts::Column::ActivityImportId.eq(import.id))
            .one(&db)
            .await
            .expect("load stored artifact")
            .expect("artifact exists");
        assert_eq!(
            artifact.artifact_kind,
            ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL
        );
        assert_eq!(
            artifact.source_quality,
            ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL
        );
        assert_eq!(artifact.storage_path, import.storage_path);

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
    async fn processed_activity_import_emits_stage_trace_events() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let processed = persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(None),
            "manual_upload",
            &training_profile,
        )
        .await
        .expect("process upload");
        let PersistActivityUploadOutcome::Imported(imported) = processed else {
            panic!("expected imported activity");
        };

        let events = integration_events::Entity::find()
            .filter(integration_events::Column::Provider.eq(ACTIVITY_PROCESSING_PROVIDER))
            .all(&db)
            .await
            .expect("load activity processing events");
        let event_stages = events
            .iter()
            .filter(|event| event.event_type == "stage_completed")
            .filter_map(|event| {
                event
                    .payload
                    .as_ref()
                    .filter(|payload| {
                        payload.get("import_id").and_then(|value| value.as_i64())
                            == Some(i64::from(imported.import.id))
                    })
                    .and_then(|payload| payload.get("stage"))
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .collect::<Vec<_>>();

        for graph_node in activity_processing_graph_nodes() {
            assert!(
                event_stages.iter().any(|stage| stage == graph_node.stage),
                "missing stage event for {}",
                graph_node.stage
            );
        }
        mark_activity_imports_processed(&db, &[imported.import.id])
            .await
            .expect("mark processed");
        let events = integration_events::Entity::find()
            .filter(integration_events::Column::Provider.eq(ACTIVITY_PROCESSING_PROVIDER))
            .all(&db)
            .await
            .expect("reload activity processing events");
        assert!(events.iter().any(|event| {
            event.event_type == "import_processed"
                && event
                    .payload
                    .as_ref()
                    .and_then(|payload| payload.get("import_id"))
                    .and_then(|value| value.as_i64())
                    == Some(i64::from(imported.import.id))
        }));

        let _ = std::fs::remove_dir_all(&uploads_dir);
    }

    #[tokio::test]
    async fn parsing_artifact_selection_prefers_original_fit_over_generated_tcx() {
        let db = test_db().await;
        let now = Utc::now();
        let import = activity_imports::ActiveModel {
            user_id: Set(1),
            source: Set("strava_sync".to_string()),
            format: Set("tcx".to_string()),
            status: Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string()),
            activity_id: Set(None),
            processing_stage: Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.to_string()),
            processing_error: Set(None),
            processing_attempts: Set(0),
            processed_at: Set(None),
            last_processing_event_at: Set(Some(now)),
            original_filename: Set("generated.tcx".to_string()),
            storage_path: Set("activity-imports/test/generated.tcx".to_string()),
            size_bytes: Set(123),
            mime_type: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert import");

        for (artifact_kind, format, source_quality, filename, storage_path) in [
            (
                ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
                "tcx",
                ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
                "generated.tcx",
                "activity-imports/test/generated.tcx",
            ),
            (
                ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL,
                "fit",
                ACTIVITY_IMPORT_SOURCE_QUALITY_FIT_ORIGINAL,
                "original.fit",
                "activity-imports/test/original.fit",
            ),
        ] {
            activity_import_artifacts::ActiveModel {
                activity_import_id: Set(import.id),
                user_id: Set(import.user_id),
                artifact_kind: Set(artifact_kind.to_string()),
                format: Set(format.to_string()),
                source_quality: Set(source_quality.to_string()),
                original_filename: Set(filename.to_string()),
                storage_path: Set(storage_path.to_string()),
                size_bytes: Set(123),
                mime_type: Set(None),
                checksum_sha256: Set("checksum".to_string()),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insert artifact");
        }

        let selected = load_best_activity_parsing_artifact(&db, &import)
            .await
            .expect("select parsing artifact");

        assert_eq!(selected.original_filename, "original.fit");
        assert_eq!(selected.format, "fit");
        assert_eq!(selected.storage_path, "activity-imports/test/original.fit");
    }

    #[tokio::test]
    async fn parsing_artifact_selection_prefers_strava_provider_payload_over_generated_tcx() {
        let db = test_db().await;
        let now = Utc::now();
        let import = activity_imports::ActiveModel {
            user_id: Set(1),
            source: Set("strava_sync".to_string()),
            format: Set("tcx".to_string()),
            status: Set(ACTIVITY_IMPORT_STATUS_PROCESSING.to_string()),
            activity_id: Set(None),
            processing_stage: Set(ACTIVITY_IMPORT_STAGE_RAW_STORED.to_string()),
            processing_error: Set(None),
            processing_attempts: Set(0),
            processed_at: Set(None),
            last_processing_event_at: Set(Some(now)),
            original_filename: Set("generated.tcx".to_string()),
            storage_path: Set("activity-imports/test/generated.tcx".to_string()),
            size_bytes: Set(123),
            mime_type: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert import");

        for (artifact_kind, format, source_quality, filename, storage_path) in [
            (
                ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
                "tcx",
                ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
                "generated.tcx",
                "activity-imports/test/generated.tcx",
            ),
            (
                ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD,
                "json",
                ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS,
                "strava_activity_99.json",
                "activity-imports/test/strava_activity_99.json",
            ),
        ] {
            activity_import_artifacts::ActiveModel {
                activity_import_id: Set(import.id),
                user_id: Set(import.user_id),
                artifact_kind: Set(artifact_kind.to_string()),
                format: Set(format.to_string()),
                source_quality: Set(source_quality.to_string()),
                original_filename: Set(filename.to_string()),
                storage_path: Set(storage_path.to_string()),
                size_bytes: Set(123),
                mime_type: Set(None),
                checksum_sha256: Set("checksum".to_string()),
                ..Default::default()
            }
            .insert(&db)
            .await
            .expect("insert artifact");
        }

        let selected = load_best_activity_parsing_artifact(&db, &import)
            .await
            .expect("select parsing artifact");

        assert_eq!(
            selected.artifact_kind,
            ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD
        );
        assert_eq!(
            selected.source_quality,
            ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS
        );
        assert_eq!(selected.original_filename, "strava_activity_99.json");
        assert_eq!(selected.format, "json");
    }

    #[tokio::test]
    async fn stored_activity_upload_import_marks_duplicates_without_new_activity() {
        let db = test_db().await;
        let uploads_dir = test_uploads_dir();
        let training_profile = TrainingProfile::default();

        let first = match persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(None),
            "manual_upload",
            &training_profile,
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

        let first = persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(Some("strava-123")),
            "strava_sync",
            &training_profile,
        )
        .await
        .expect("import first Strava upload");
        assert!(matches!(first, PersistActivityUploadOutcome::Imported(_)));

        let second = persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(Some("strava-123")),
            "strava_sync",
            &training_profile,
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

        let imported = match persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(None),
            "manual_upload",
            &training_profile,
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

        let imported = match persist_test_activity_upload(
            &db,
            &uploads_dir,
            fit_upload(None),
            "manual_upload",
            &training_profile,
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
