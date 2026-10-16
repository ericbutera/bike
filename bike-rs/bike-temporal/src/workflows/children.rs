use super::options;
use bike_core::bike_tasks::BatchEntry;
use temporalio_common::data_converters::TemporalDeserializable;
use temporalio_sdk::{
    ChildWorkflowCancellationType, ChildWorkflowOptions, WorkflowContext, WorkflowResult,
};
use temporalio_workflow::common::WorkflowDefinition;

/// One typed child call owns identity, matching options and error adaptation.
pub(super) async fn run<W, D>(
    ctx: &WorkflowContext<W>,
    definition: D,
    input: D::Input,
) -> WorkflowResult<D::Output>
where
    D: WorkflowDefinition + 'static,
    D::Input: BatchEntry,
    D::Output: TemporalDeserializable,
{
    options::workflow_result(ctx, Ok(()))?;
    let child_options = ChildWorkflowOptions::builder()
        .workflow_id(format!(
            "bike-task-{}-{}",
            definition.name(),
            input.command_id()
        ))
        .task_queue(crate::TASK_QUEUE.to_owned())
        .priority(options::priority(
            input.priority(),
            format!("user-{}", input.user_id()),
        ))
        .cancel_type(ChildWorkflowCancellationType::WaitCancellationCompleted)
        .build();
    let child = ctx
        .start_child_workflow(definition, input, child_options)
        .await
        .map_err(|error| {
            if ctx.cancellation_token().is_cancelled() {
                temporalio_sdk::WorkflowTermination::cancelled()
            } else {
                error.into()
            }
        })?;
    let result = child.result().await.map_err(|error| {
        if error.as_cancelled().is_some() {
            temporalio_sdk::WorkflowTermination::cancelled()
        } else {
            error.into()
        }
    });
    options::workflow_result(ctx, result)
}
