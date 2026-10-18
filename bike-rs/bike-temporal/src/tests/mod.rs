use crate::workflows::*;
use bike_core::bike_tasks::*;
struct BikeWorkflowAssertions;
impl workflow_test_support::WorkflowAssertions for BikeWorkflowAssertions {
    fn child(
        input: &serde_json::Value,
        command: &temporalio_common::protos::coresdk::workflow_commands::StartChildWorkflowExecution,
    ) {
        assert_eq!(command.task_queue, crate::TASK_QUEUE);
        let owner = input
            .get("user_id")
            .or_else(|| input.get("source").and_then(|source| source.get("user_id")));
        if let Some(owner) = owner {
            assert_eq!(
                serde_json::from_slice::<String>(
                    &command.search_attributes.as_ref().unwrap().indexed_fields["BikeUserId"].data
                )
                .unwrap(),
                owner.to_string()
            );
        }
    }
}
type WorkflowMock = workflow_test_support::WorkflowMock<BikeWorkflowAssertions>;
mod batches;
mod client;
mod composition;
mod fixtures;
mod handlers;
mod import_operations;
mod imports;
mod operations;
mod submission;
