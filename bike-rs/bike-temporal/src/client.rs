use crate::{
    workflows::{options, ProcessActivityImport, ReprocessActivityImport},
    TASK_QUEUE,
};
use bike_core::bike_tasks::{ActivityImport, ImportSource, ImportTarget, JobReference, Priority, ReprocessInput};
use temporalio_client::{
    errors::WorkflowStartError, Client, NamespacedClient, WorkflowIdConflictPolicy, WorkflowIdReusePolicy,
    WorkflowStartOptions,
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

fn command_options(id: String, priority: Priority, user_id: i32) -> WorkflowStartOptions {
    WorkflowStartOptions::new(TASK_QUEUE, id)
        .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
        .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
        .priority(options::priority(priority, format!("user-{user_id}")))
        .build()
}

pub(crate) async fn process_activity_import(client: &Client, source: ImportSource) -> Result<JobReference, Error> {
    ActivityImport {
        source: source.clone(),
        target: ImportTarget::NewActivity,
    }
    .validate()?;
    let id = format!(
        "bike-task-process_activity_import-user-{}-import-{}-{}",
        source.user_id, source.activity_import_id, source.command_id
    );
    let options = command_options(id.clone(), source.priority, source.user_id);
    let result = client
        .start_workflow(ProcessActivityImport::run, source, options)
        .await
        .map(|handle| handle.run_id().map(str::to_owned));
    job_reference(client, id, "process_activity_import", result)
}

pub(crate) async fn reprocess_activity_import(client: &Client, input: ReprocessInput) -> Result<JobReference, Error> {
    input.clone().into_command().validate()?;
    // Bind the job to the target, so reusing a command on a different activity cannot return an unrelated job.
    let id = format!(
        "bike-task-reprocess_activity_import-user-{}-activity-{}-import-{}-{}",
        input.source.user_id, input.activity_id, input.source.activity_import_id, input.source.command_id
    );
    let options = command_options(id.clone(), input.source.priority, input.source.user_id);
    let result = client
        .start_workflow(ReprocessActivityImport::run, input, options)
        .await
        .map(|handle| handle.run_id().map(str::to_owned));
    job_reference(client, id, "reprocess_activity_import", result)
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
