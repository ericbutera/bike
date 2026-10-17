use super::options;
use bike_core::bike_tasks::Priority;
use serde::Serialize;
use temporalio_common::data_converters::TemporalDeserializable;
use temporalio_sdk::{ChildWorkflowCancellationType, ChildWorkflowOptions, WorkflowContext, WorkflowResult};
use temporalio_workflow::common::WorkflowDefinition;

/// A normal request-owned child. The parent awaits its result.
pub(super) async fn run<W, D>(
    ctx: &WorkflowContext<W>,
    definition: D,
    input: D::Input,
    priority: Priority,
    resource: String,
) -> WorkflowResult<D::Output>
where
    D: WorkflowDefinition + 'static,
    D::Input: Serialize,
    D::Output: TemporalDeserializable,
{
    if ctx.cancellation_token().is_cancelled() {
        return Err(temporalio_sdk::WorkflowTermination::cancelled());
    }
    let child_options = ChildWorkflowOptions::builder()
        .task_queue(crate::TASK_QUEUE.to_owned())
        .priority(options::priority(priority, resource))
        .cancel_type(ChildWorkflowCancellationType::WaitCancellationCompleted)
        .build();
    let child = ctx.start_child_workflow(definition, input, child_options).await?;
    child.result().await.map_err(|error| {
        if error.as_cancelled().is_some() {
            temporalio_sdk::WorkflowTermination::cancelled()
        } else {
            error.into()
        }
    })
}
