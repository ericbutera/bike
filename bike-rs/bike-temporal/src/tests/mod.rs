use crate::workflows::*;
use bike_core::bike_tasks::*;
struct BikeWorkflowAssertions;
impl workflow_test_support::WorkflowAssertions for BikeWorkflowAssertions {
    fn child(
        _input: &serde_json::Value,
        command: &temporalio_common::protos::coresdk::workflow_commands::StartChildWorkflowExecution,
    ) {
        assert_eq!(command.task_queue, crate::TASK_QUEUE);
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
