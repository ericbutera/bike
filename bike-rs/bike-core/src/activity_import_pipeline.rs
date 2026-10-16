use crate::activity_import_execution::{self as execution, ProcessingCheckpoint, StageRecord};
use crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds;
use crate::activity_parser::{parse_activity_artifact, ActivityParserArtifact, ParsedActivityData};
use crate::activity_training_analysis::rebuild_activity_training_analysis_cache;
use crate::activity_type::ActivityType;
use crate::analytics::{rebuild_activity_analytics_cache, rebuild_segment_analytics_cache};
use crate::dedupe::activity_dedupe_matches_model;
use crate::entities::activities::parsed::{build_save_data, ActivitySaveData};
use crate::entities::{
    activities, activity_import_artifacts, activity_import_attempts, activity_imports,
    activity_training_analyses,
};
use crate::integration_events_service::INTEGRATION_LEVEL_INFO;
use crate::jobs::JobQueue as TaskQueue;
use crate::object_store::{ObjectOwner, ObjectRef, ObjectStore};
use crate::training_profile::{load_training_profile, TrainingProfile};
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, NaiveDate, Utc};
use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use uuid::Uuid;
mod replay;
mod stage_execution;
use self::stage_execution::{
    begin_processing_attempt, complete_processing_node, start_processing_node,
};
pub use replay::{
    plan_activity_import_replay, queue_activity_import_replay,
    queue_expected_activity_import_replay, ActivityImportReplayPlan,
};

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

pub enum PersistActivityUploadOutcome {
    Imported(PersistedActivityImport),
    Duplicate(DeduplicatedActivityImport),
}

pub use crate::activity_import_lifecycle::{
    ACTIVITY_IMPORT_STAGE_ACTIVITY_ANALYTICS_BUILT, ACTIVITY_IMPORT_STAGE_ACTIVITY_PARSED,
    ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED, ACTIVITY_IMPORT_STAGE_COMPLETE,
    ACTIVITY_IMPORT_STAGE_RAW_STORED, ACTIVITY_IMPORT_STAGE_SEGMENTS_BUILT,
    ACTIVITY_IMPORT_STAGE_SEGMENT_ANALYTICS_BUILT, ACTIVITY_IMPORT_STAGE_TRAINING_ANALYSIS_BUILT,
    ACTIVITY_IMPORT_STALE_PROCESSING_SECONDS, ACTIVITY_IMPORT_STATUS_DUPLICATE,
    ACTIVITY_IMPORT_STATUS_FAILED, ACTIVITY_IMPORT_STATUS_PROCESSED,
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
pub const ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE: i32 =
    activity_imports::ACTIVITY_IMPORT_VERSION_ARTIFACT_AWARE;
pub const ACTIVITY_IMPORT_VERSION_CURRENT: i32 = activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT;
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
    crate::activity_import_recovery::recover_stale_manual_activity_imports(db, tasks, now)
        .await
        .map_err(|error| AppError::internal(error.message))
}

pub async fn recover_abandoned_manual_activity_imports_after_worker_start(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    crate::activity_import_recovery::recover_abandoned_manual_activity_imports_after_worker_start(
        db, tasks, now,
    )
    .await
    .map_err(|error| AppError::internal(error.message))
}

pub async fn recover_stale_manual_activity_imports_for_user(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    now: DateTime<Utc>,
) -> Result<usize, AppError> {
    crate::activity_import_recovery::recover_stale_manual_activity_imports_for_user(
        db, tasks, user_id, now,
    )
    .await
    .map_err(|error| AppError::internal(error.message))
}

pub fn infer_activity_type(title: &str, original_filename: &str) -> ActivityType {
    ActivityType::infer(title, original_filename)
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
}

struct ActivityProcessingState {
    attempt: activity_import_attempts::Model,
    import_model: activity_imports::Model,
    activity_model: Option<activities::Model>,
    parsed_activity: Option<ParsedActivityData>,
    affected_segment_ids: Vec<i32>,
    parsing_artifact: ActivityProcessingArtifact,
    bytes: Vec<u8>,
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
        let sha256 = self.checksum_sha256.clone().ok_or_else(|| {
            AppError::bad_request("Retained source needs a checksum before native reprocessing")
        })?;
        Ok(ObjectRef {
            path: self.storage_path.clone(),
            sha256,
            bytes: self
                .size_bytes
                .try_into()
                .map_err(|_| AppError::bad_request("Invalid retained source size"))?,
        })
    }
}

pub async fn mark_activity_import_processing_stage(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    stage: &str,
    activity_id: Option<i32>,
) -> Result<activity_imports::Model, AppError> {
    crate::activity_import_lifecycle::mark_activity_import_processing_stage(
        db,
        import,
        stage,
        activity_id,
    )
    .await
    .map_err(|error| AppError::internal(error.message))
}

pub async fn mark_activity_imports_processed(
    db: &DatabaseConnection,
    import_ids: &[i32],
) -> Result<(), AppError> {
    crate::activity_import_lifecycle::mark_activity_imports_processed(db, import_ids)
        .await
        .map_err(|error| AppError::internal(error.message))
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

pub async fn mark_activity_import_duplicate(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
    duplicate_activity_id: i32,
) -> Result<activity_imports::Model, AppError> {
    crate::activity_import_lifecycle::mark_activity_import_duplicate(
        db,
        import,
        duplicate_activity_id,
    )
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
    crate::activity_import_lifecycle::record_activity_processing_event(
        db, import, event_type, level, message, payload,
    )
    .await;
}

async fn run_activity_processing_graph(
    run: ActivityProcessingRun<'_>,
) -> Result<PersistActivityUploadOutcome, AppError> {
    run.import.require_sql_processing()?;
    let attempt = begin_processing_attempt(&run).await?;
    let result = execute_activity_processing_graph(&run, attempt.clone()).await;
    if let Err(error) = &result {
        let own_attempt =
            activity_import_attempts::Entity::find_for_execution(run.db, attempt.id).await?;
        if let Some(own_attempt) = own_attempt.filter(|attempt| attempt.status == "running") {
            mark_activity_import_failed(run.db, &run.import, &own_attempt.current_stage, error)
                .await?;
        }
    }
    result
}

async fn execute_activity_processing_graph(
    run: &ActivityProcessingRun<'_>,
    attempt: activity_import_attempts::Model,
) -> Result<PersistActivityUploadOutcome, AppError> {
    let graph_nodes_by_node = activity_processing_graph_nodes()
        .iter()
        .map(|node| (node.node, *node))
        .collect::<HashMap<_, _>>();
    let mut state = load_activity_processing_state(run, attempt).await?;

    for node in activity_processing_topological_order()? {
        let graph_node = graph_nodes_by_node.get(&node).ok_or_else(|| {
            AppError::internal(format!(
                "Activity processing graph is missing metadata for {node:?}"
            ))
        })?;

        if execution::stages(&state.attempt)?
            .iter()
            .any(|stage| stage.stage == graph_node.stage && stage.status == "reused")
        {
            continue;
        }
        start_processing_node(run, &mut state, graph_node.stage).await?;
        let outcome = run_activity_processing_node(run, &mut state, *graph_node).await?;
        complete_processing_node(run, &mut state, node).await?;
        if let Some(outcome) = outcome {
            execution::finish_active(run.db, &state.import_model, "duplicate", None).await?;
            return Ok(outcome);
        }
    }

    activity_processing_result(state)
}

async fn load_activity_processing_state(
    run: &ActivityProcessingRun<'_>,
    attempt: activity_import_attempts::Model,
) -> Result<ActivityProcessingState, AppError> {
    let parsing_artifact = load_best_activity_parsing_artifact(run.db, &run.import).await?;
    let bytes =
        tokio::fs::read(Path::new(run.uploads_dir).join(&parsing_artifact.storage_path)).await?;
    if let Some(checksum) = &parsing_artifact.checksum_sha256 {
        if checksum_sha256_hex(&bytes) != *checksum {
            return Err(AppError::bad_request(
                "Retained source checksum does not match; replay is blocked",
            ));
        }
    }
    let checkpoint = execution::checkpoint(&attempt);
    if attempt.reused_attempt_id.is_some()
        && (attempt.source_json != stage_execution::artifact_metadata(&parsing_artifact)
            || checkpoint.as_ref().is_some_and(|checkpoint| {
                run.existing_activity.as_ref().is_some_and(|activity| {
                    checkpoint.activity_updated_at != Some(activity.updated_at)
                })
            }))
    {
        return Err(AppError::conflict(
            "Earlier results changed after replay was queued; create a new replay",
        ));
    }
    Ok(ActivityProcessingState {
        attempt,
        import_model: run.import.clone(),
        activity_model: run.existing_activity.clone(),
        parsed_activity: checkpoint.as_ref().map(|value| value.parsed.clone()),
        affected_segment_ids: checkpoint.map_or_else(Vec::new, |value| value.affected_segment_ids),
        parsing_artifact,
        bytes,
    })
}

pub(crate) async fn load_best_activity_parsing_artifact(
    db: &DatabaseConnection,
    import: &activity_imports::Model,
) -> Result<ActivityProcessingArtifact, AppError> {
    let artifacts =
        activity_import_artifacts::Entity::for_import(db, import.user_id, import.id).await?;

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
        size_bytes: artifact.size_bytes,
        checksum_sha256: Some(artifact.checksum_sha256),
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
    if !activity_format_is_parsable(&import.format)
        || (import.source == "strava_sync" && import.format == "tcx")
    {
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
        (ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL, "strava_archive_json", "json") => 350,
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
    let mut parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &state.parsing_artifact.original_filename,
        format: &state.parsing_artifact.format,
        artifact_kind: &state.parsing_artifact.artifact_kind,
        source_quality: &state.parsing_artifact.source_quality,
        bytes: &state.bytes,
    })?;
    enrich_recording(
        run.db,
        &ObjectStore::filesystem(run.uploads_dir, ObjectOwner::ActivityImports),
        &state.import_model,
        &state.parsing_artifact,
        state.activity_model.as_ref(),
        &mut parsed,
    )
    .await?;
    state.parsed_activity = Some(parsed);
    state.import_model = mark_activity_import_processing_stage(
        run.db,
        &state.import_model,
        stage,
        state.activity_model.as_ref().map(|activity| activity.id),
    )
    .await?;
    Ok(())
}

/// Preserve the same recording/privacy classification on legacy and native parses.
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
        parsed
            .derived_data
            .recording
            .merge(activity.recording_context());
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
    let artifacts =
        activity_import_artifacts::Entity::for_import(db, import.user_id, import.id).await?;
    for artifact in artifacts.into_iter().filter(|a| {
        a.storage_path != parsing_artifact.storage_path && activity_artifact_is_parsable(a)
    }) {
        let artifact = activity_processing_artifact_from_model(artifact);
        let bytes = objects.read(&artifact.object_reference()?).await?;
        let other = parse_activity_artifact(ActivityParserArtifact {
            original_filename: &artifact.original_filename,
            format: &artifact.format,
            artifact_kind: &artifact.artifact_kind,
            source_quality: &artifact.source_quality,
            bytes: &bytes,
        })?;
        parsed
            .derived_data
            .recording
            .merge(other.derived_data.recording);
    }
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

    let activity =
        merge_duplicate_recording(run.db, duplicate.activity, &parsed.derived_data.recording)
            .await?;
    let import_model =
        mark_activity_import_duplicate(run.db, &state.import_model, activity.id).await?;
    Ok(Some(PersistActivityUploadOutcome::Duplicate(
        DeduplicatedActivityImport {
            activity,
            existing_import: Some(import_model),
        },
    )))
}

async fn merge_duplicate_recording(
    db: &DatabaseConnection,
    activity: activities::Model,
    recording: &crate::activity_recording::RecordingContext,
) -> Result<activities::Model, AppError> {
    let mut derived = crate::activity_data::deserialize_derived_activity_data(
        activity.derived_data_json.as_ref(),
    );
    let previous = derived.recording.clone();
    derived.recording.merge(recording.clone());
    if derived.recording != previous {
        if !activities::Model::store_recording(
            db,
            activity.id,
            activity.user_id,
            activity.updated_at,
            &derived,
        )
        .await?
        {
            return Err(AppError::internal(
                "Duplicate activity changed while merging recording evidence; retry the import",
            ));
        }
        let refreshed = activities::Model::find_owned(db, activity.id, activity.user_id)
            .await?
            .ok_or_else(|| AppError::internal("Duplicate activity disappeared"))?;
        crate::activity_recording_recovery::propagate_recording(db, &refreshed).await?;
        return Ok(refreshed);
    }
    Ok(activity)
}

async fn save_activity_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let parsed = parsed_activity_ref(state)?.clone();
    let training_profile = load_processing_training_profile(run).await?;
    let save_data = build_save_data(
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

    crate::activity_recording_recovery::propagate_recording(run.db, &saved_activity).await?;

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

async fn update_activity_from_parsed(
    db: &DatabaseConnection,
    activity: activities::Model,
    parsing_artifact: &ActivityProcessingArtifact,
    parsed: &ParsedActivityData,
    save_data: ActivitySaveData,
) -> Result<activities::Model, AppError> {
    activity
        .update_parsed(db, parsing_artifact, parsed, save_data)
        .await
}

async fn insert_activity_from_parsed(
    run: &ActivityProcessingRun<'_>,
    import: &activity_imports::Model,
    parsing_artifact: &ActivityProcessingArtifact,
    parsed: &ParsedActivityData,
    save_data: ActivitySaveData,
) -> Result<activities::Model, AppError> {
    activities::Model::insert_parsed(
        run.db,
        import,
        parsing_artifact,
        parsed,
        save_data,
        run.source_correlation_id.clone(),
    )
    .await
}

async fn build_segments_node(
    run: &ActivityProcessingRun<'_>,
    state: &mut ActivityProcessingState,
    stage: &str,
) -> Result<(), AppError> {
    let parsed = parsed_activity_ref(state)?;
    let activity = activity_model_ref(state)?;
    let activity_id = activity.id;
    let route_points = if activity.is_bike_activity() {
        parsed.derived_data.route_points.as_slice()
    } else {
        &[]
    };
    state.affected_segment_ids = refresh_activity_derived_state_without_cache_rebuilds(
        run.db,
        run.user_id,
        activity_id,
        route_points,
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
    if !activity_model_ref(state)?.is_bike_activity() {
        activity_training_analyses::Model::clear_activity(run.db, activity_id).await?;
        state.import_model = mark_activity_import_processing_stage(
            run.db,
            &state.import_model,
            stage,
            Some(activity_id),
        )
        .await?;
        return Ok(());
    }

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

#[tracing::instrument(name = "activity.import", skip_all, fields(bike.operation = "activity.import", user_id = request.user_id, source = request.source))]
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
            let recording = incoming_recording(&request)?;
            let activity = merge_duplicate_recording(db, existing.activity, &recording).await?;
            return Ok(PersistActivityUploadOutcome::Duplicate(
                DeduplicatedActivityImport {
                    activity,
                    existing_import: existing.existing_import,
                },
            ));
        }
    }

    let import_model = store_activity_upload_import_with_artifacts(
        db,
        StoreActivityUploadImportRequest {
            uploads_dir: request.uploads_dir,
            user_storage_key: request.user_storage_key,
            user_id: request.user_id,
            upload: request.upload,
            primary_artifact_kind: request.primary_artifact_kind,
            primary_source_quality: request.primary_source_quality,
            additional_artifacts: request.additional_artifacts,
            source: request.source,
        },
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
    })
    .await;

    result
}

fn incoming_recording(
    request: &PersistActivityUploadWithArtifactsRequest<'_>,
) -> Result<crate::activity_recording::RecordingContext, AppError> {
    let mut recording = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &request.upload.original_filename,
        format: &request.upload.format,
        artifact_kind: request.primary_artifact_kind,
        source_quality: request.primary_source_quality,
        bytes: &request.upload.bytes,
    })?
    .derived_data
    .recording;
    for artifact in &request.additional_artifacts {
        if !activity_format_is_parsable(&artifact.format)
            && !(artifact.artifact_kind == ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD
                && artifact.source_quality == ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS
                && artifact.format == "json")
        {
            continue;
        }
        recording.merge(
            parse_activity_artifact(ActivityParserArtifact {
                original_filename: &artifact.original_filename,
                format: &artifact.format,
                artifact_kind: &artifact.artifact_kind,
                source_quality: &artifact.source_quality,
                bytes: &artifact.bytes,
            })?
            .derived_data
            .recording,
        );
    }
    Ok(recording)
}

pub async fn find_stored_activity_import(
    db: &DatabaseConnection,
    user_id: i32,
    bytes: &[u8],
) -> Result<Option<activity_imports::Model>, AppError> {
    activity_import_artifacts::Entity::find_existing_original_by_checksum(
        db,
        user_id,
        &checksum_sha256_hex(bytes),
    )
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
        activity_imports::Model::store_upload(db, user_id, source, &upload, relative_path.clone())
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

async fn store_activity_upload_source(
    request: &StoreActivityUploadImportRequest<'_>,
) -> Result<String, AppError> {
    if request.source == "strava_sync"
        && request.primary_artifact_kind == ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD
    {
        return Ok(
            crate::strava::objects::ObjectStore::new(request.uploads_dir)
                .write(
                    &format!("strava/sources/{}", request.user_id),
                    &request.upload.bytes,
                )
                .await?
                .path,
        );
    }
    let relative =
        activity_import_storage_path(request.user_storage_key, &request.upload.format, Utc::now());
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
    db: &impl sea_orm::ConnectionTrait,
    import: &activity_imports::Model,
    artifact: ActivityImportArtifactPayload,
    storage_path: String,
    size_bytes: i64,
) -> Result<activity_import_artifacts::Model, AppError> {
    activity_import_artifacts::Model::store_artifact(db, import, artifact, storage_path, size_bytes)
        .await
}

pub(crate) fn checksum_sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[tracing::instrument(name = "activity.import.process", skip_all, fields(bike.operation = "activity.import.process", user_id, import_id = import.id))]
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
    })
    .await
}

pub async fn finalize_activity_import_batch(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    user_id: i32,
    affected_segment_ids: Vec<i32>,
    fitness_dirty_from_day: Option<NaiveDate>,
    changed_at: DateTime<Utc>,
) -> Result<(), AppError> {
    crate::activity_import_lifecycle::finalize_activity_import_batch(
        db,
        tasks,
        user_id,
        affected_segment_ids,
        fitness_dirty_from_day,
        changed_at,
    )
    .await
    .map_err(|error| AppError::internal(error.message))
}

pub async fn reprocess_activity_from_import(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
    activity_import: activity_imports::Model,
    training_profile: Option<&TrainingProfile>,
) -> Result<ReprocessedActivityImport, AppError> {
    let outcome = run_activity_processing_graph(ActivityProcessingRun {
        db,
        uploads_dir,
        user_id,
        import: activity_import,
        existing_activity: Some(activity),
        source_correlation_id: None,
        deduplication: ActivityUploadDeduplication::Disabled,
        training_profile,
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
    let sport_values = crate::activity_sport::activity_sport_from_stored(&activity_draft.sport)
        .map(|sport| sport.stored_values().to_vec())
        .unwrap_or_else(|| vec![activity_draft.sport.as_str()]);
    let candidates = activities::Model::at_start_for_sports(
        db,
        user_id,
        activity_draft.started_at,
        &sport_values,
    )
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

    let activity =
        activities::Model::from_source_identity(db, user_id, source, source_correlation_id).await?;

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
        Some(import_id) => activity_imports::Entity::find_for_administration(db, import_id).await?,
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
}
