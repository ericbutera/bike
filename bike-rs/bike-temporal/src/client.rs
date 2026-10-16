use crate::workflows::options;
use crate::{
    workflows::{
        DebouncePolicy, PrepareHeatmap, ProcessActivityImport, RebuildFitnessFreshness,
        RebuildInput, RebuildSegmentAnalytics, ReprocessActivityImport, ReprocessInput,
    },
    TASK_QUEUE,
};
use bike_core::bike_tasks::{
    validate_revision, ActivityImport, BatchEntry, FitnessChange, HeatmapChange, ImportSource,
    ImportTarget, JobReference, Priority, SegmentChange,
};
use temporalio_client::{
    errors::WorkflowStartError, Client, NamespacedClient, WorkflowIdConflictPolicy,
    WorkflowIdReusePolicy, WorkflowStartOptions,
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

pub(crate) fn command_workflow_id(workflow_type: &str, user_id: i32, command_id: &str) -> String {
    format!("bike-task-{workflow_type}-user-{user_id}-{command_id}")
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

fn command_options(id: String, priority: Priority, user_id: i32) -> WorkflowStartOptions {
    WorkflowStartOptions::new(TASK_QUEUE, id)
        .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
        .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
        .priority(options::priority(priority, format!("user-{user_id}")))
        .build()
}

fn job_reference(
    client: &Client,
    job_id: String,
    workflow_type: &str,
    result: Result<Option<String>, WorkflowStartError>,
) -> Result<JobReference, Error> {
    let run_id = match result {
        Ok(run_id) | Err(WorkflowStartError::AlreadyStarted { run_id, .. }) => run_id,
        Err(error) => return Err(error.into()),
    };
    Ok(JobReference {
        job_id,
        namespace: client.namespace(),
        run_id: run_id.ok_or("Temporal did not return an execution run ID")?,
        workflow_type: workflow_type.into(),
    })
}

pub(crate) async fn process_activity_import(
    client: &Client,
    source: ImportSource,
) -> Result<JobReference, Error> {
    ActivityImport {
        source: source.clone(),
        target: ImportTarget::NewActivity,
    }
    .validate()?;
    start_command(client, ProcessActivityImport::run, source).await
}

pub(crate) async fn reprocess_activity_import(
    client: &Client,
    input: ReprocessInput,
) -> Result<JobReference, Error> {
    ActivityImport {
        source: input.source.clone(),
        target: ImportTarget::ExistingActivity {
            activity_id: input.activity_id,
            expected_revision: input.expected_revision,
        },
    }
    .validate()?;
    start_command(client, ReprocessActivityImport::run, input).await
}

async fn start_command<D>(
    client: &Client,
    definition: D,
    input: D::Input,
) -> Result<JobReference, Error>
where
    D: temporalio_common::HasWorkflowDefinition,
    D::Input: BatchEntry + Send,
{
    let name = definition.name().to_owned();
    let job_id = command_workflow_id(&name, input.user_id(), input.command_id());
    let options = command_options(job_id.clone(), input.priority(), input.user_id());
    let result = client
        .start_workflow(definition, input, options)
        .await
        .map(|handle| handle.run_id().map(str::to_owned));
    job_reference(client, job_id, &name, result)
}
