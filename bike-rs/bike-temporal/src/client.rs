use crate::workflows::options;
use crate::{
    workflows::{
        DebouncePolicy, PrepareHeatmap, ProcessActivityImport, RebuildFitnessFreshness,
        RebuildInput, RebuildSegmentAnalytics, ReprocessActivityImport, ReprocessInput,
    },
    TASK_QUEUE,
};
use bike_core::bike_tasks::{
    validate_revision, ActivityImport, FitnessChange, HeatmapChange, ImportSource, ImportTarget,
    Priority, SegmentChange,
};
use temporalio_client::{
    Client, WorkflowIdConflictPolicy, WorkflowIdReusePolicy, WorkflowStartOptions,
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub fn fitness_workflow_id(user_id: i32) -> String {
    format!("bike-task-rebuild_fitness_freshness-user-{user_id}")
}
pub fn segment_workflow_id(segment_id: i32) -> String {
    format!("bike-task-rebuild_segment_analytics-segment-{segment_id}")
}
pub fn heatmap_workflow_id(activity_id: i32) -> String {
    format!("bike-task-prepare_heatmap-activity-{activity_id}")
}

fn start_options(id: String, priority: Priority, fairness: String) -> WorkflowStartOptions {
    WorkflowStartOptions::new(TASK_QUEUE, id)
        .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
        .id_reuse_policy(WorkflowIdReusePolicy::AllowDuplicateFailedOnly)
        .priority(options::priority(priority, fairness))
        .build()
}

pub async fn request_fitness_freshness(
    client: &Client,
    change: FitnessChange,
    policy: DebouncePolicy,
) -> Result<(), Error> {
    validate_revision(change.revision)?;
    let options = start_options(
        fitness_workflow_id(change.user_id),
        change.priority,
        format!("user-{}", change.user_id),
    );
    client
        .signal_with_start_workflow(
            RebuildFitnessFreshness::run,
            RebuildInput::new(change.clone(), policy),
            RebuildFitnessFreshness::changed,
            change,
            options,
        )
        .await?;
    Ok(())
}

pub async fn request_segment_analytics(
    client: &Client,
    change: SegmentChange,
    policy: DebouncePolicy,
) -> Result<(), Error> {
    validate_revision(change.revision)?;
    let options = start_options(
        segment_workflow_id(change.segment_id),
        change.priority,
        format!("segment-{}", change.segment_id),
    );
    client
        .signal_with_start_workflow(
            RebuildSegmentAnalytics::run,
            RebuildInput::new(change.clone(), policy),
            RebuildSegmentAnalytics::changed,
            change,
            options,
        )
        .await?;
    Ok(())
}

pub async fn request_heatmap(
    client: &Client,
    change: HeatmapChange,
    policy: DebouncePolicy,
) -> Result<(), Error> {
    validate_revision(change.generation)?;
    let options = start_options(
        heatmap_workflow_id(change.activity_id),
        change.priority,
        format!("user-{}", change.user_id),
    );
    client
        .signal_with_start_workflow(
            PrepareHeatmap::run,
            RebuildInput::new(change.clone(), policy),
            PrepareHeatmap::changed,
            change,
            options,
        )
        .await?;
    Ok(())
}

pub async fn process_activity_import(client: &Client, source: ImportSource) -> Result<(), Error> {
    ActivityImport {
        source: source.clone(),
        target: ImportTarget::NewActivity,
    }
    .validate()?;
    let options = start_options(
        format!("bike-task-process_activity_import-{}", source.command_id),
        source.priority,
        format!("user-{}", source.user_id),
    );
    client
        .start_workflow(ProcessActivityImport::run, source, options)
        .await?;
    Ok(())
}

pub async fn reprocess_activity_import(
    client: &Client,
    input: ReprocessInput,
) -> Result<(), Error> {
    ActivityImport {
        source: input.source.clone(),
        target: ImportTarget::ExistingActivity {
            activity_id: input.activity_id,
            expected_revision: input.expected_revision,
        },
    }
    .validate()?;
    let options = start_options(
        format!(
            "bike-task-reprocess_activity_import-{}",
            input.source.command_id
        ),
        input.source.priority,
        format!("user-{}", input.source.user_id),
    );
    client
        .start_workflow(ReprocessActivityImport::run, input, options)
        .await?;
    Ok(())
}
