use super::QueuedJobReference;
use crate::bike_tasks::JobReference;
use std::time::Duration;
use temporalio_client::{
    errors::WorkflowStartError, Client, ClientOptions, Connection, ConnectionOptions, NamespacedClient,
    WorkflowIdConflictPolicy, WorkflowIdReusePolicy, WorkflowStartOptions,
};
use temporalio_common::{
    data_converters::{PayloadConverter, RawValue},
    UntypedWorkflow,
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn connect() -> Result<Client, Error> {
    let address = std::env::var("TEMPORAL_ADDRESS").unwrap_or_else(|_| "http://temporal:7233".into());
    let namespace = std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into());
    let options = ConnectionOptions::new(address.parse::<temporalio_client::Url>()?)
        .identity("bike-tasks")
        .connect_timeout(Duration::from_secs(5))
        .build();
    Ok(Client::new(
        Connection::connect(options).await?,
        ClientOptions::new(namespace).build(),
    )?)
}

pub async fn submit(client: &Client, name: &str, input: serde_json::Value) -> Result<QueuedJobReference, Error> {
    let id = format!("bike-{name}-{}", uuid::Uuid::new_v4());
    client
        .start_workflow(
            UntypedWorkflow::new(name),
            RawValue::from_value(&input, &PayloadConverter::default()),
            WorkflowStartOptions::new(crate::platform::temporal::BIKE_TASK_QUEUE, id.clone()).build(),
        )
        .await?;
    Ok(QueuedJobReference {
        id,
        status: "running".into(),
    })
}

pub async fn submit_command(
    client: &Client,
    task: super::MaintenanceTask,
    command_id: uuid::Uuid,
) -> Result<JobReference, Error> {
    task.validate()?;
    let workflow_type = task.workflow_type();
    let input = serde_json::to_value(task)?;
    let target_id = uuid::Uuid::new_v5(&command_id, &serde_json::to_vec(&input)?);
    let job_id = format!("bike-{workflow_type}-{target_id}");
    let options = WorkflowStartOptions::new(crate::platform::temporal::BIKE_TASK_QUEUE, job_id.clone())
        .id_conflict_policy(WorkflowIdConflictPolicy::UseExisting)
        .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
        .build();
    let result = client
        .start_workflow(
            UntypedWorkflow::new(workflow_type),
            RawValue::from_value(&input, &PayloadConverter::default()),
            options,
        )
        .await
        .map(|handle| handle.run_id().map(str::to_owned));
    let run_id = match result {
        Ok(id) | Err(WorkflowStartError::AlreadyStarted { run_id: id, .. }) => id,
        Err(error) => return Err(error.into()),
    };
    Ok(JobReference {
        job_id,
        run_id: run_id.ok_or("Temporal did not acknowledge a run ID")?,
        namespace: client.namespace(),
        workflow_type: workflow_type.into(),
    })
}
