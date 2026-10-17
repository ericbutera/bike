//! Temporal owns canonical command input, including retained closed executions.
use super::{Client, Error, JobReference, NamespacedClient, ReprocessInput};
use temporalio_client::{grpc::WorkflowService, tonic::IntoRequest};
use temporalio_common::{
    data_converters::{SerializationContextData, WorkflowSerializationContext},
    protos::temporal::api::{
        common::v1::WorkflowExecution, history::v1::history_event::Attributes,
        workflowservice::v1::GetWorkflowExecutionHistoryRequest,
    },
};

pub(super) async fn validate_accepted_input(
    client: &Client,
    job: &JobReference,
    input: &ReprocessInput,
) -> Result<(), Error> {
    let response = client
        .clone()
        .get_workflow_execution_history(
            GetWorkflowExecutionHistoryRequest {
                namespace: client.namespace(),
                execution: Some(WorkflowExecution {
                    workflow_id: job.job_id.clone(),
                    run_id: job.run_id.clone(),
                }),
                maximum_page_size: 1,
                // A start event is durable before StartWorkflowExecution returns; never long poll.
                wait_new_event: false,
                ..Default::default()
            }
            .into_request(),
        )
        .await?
        .into_inner();
    let event = response
        .history
        .and_then(|history| history.events.into_iter().next())
        .ok_or("Temporal did not return the retained command start event")?;
    let Some(Attributes::WorkflowExecutionStartedEventAttributes(start)) = event.attributes else {
        return Err("Temporal did not return the retained command start event".into());
    };
    if start.workflow_type.as_ref().map(|kind| kind.name.as_str()) != Some(job.workflow_type.as_str()) {
        return Err("Temporal command workflow type does not match".into());
    }
    let payloads = start.input.ok_or("Temporal command input is missing")?.payloads;
    let original: ReprocessInput = client
        .data_converter()
        .from_payloads(
            &SerializationContextData::Workflow(WorkflowSerializationContext::new()),
            payloads,
        )
        .await?;
    input.validate_retry(&original)?;
    Ok(())
}
