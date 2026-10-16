use bike_core::bike_tasks::Priority as WorkPriority;
use bike_core::platform::temporal as policy;
use serde::Serialize;
use temporalio_common::{data_converters::TemporalDeserializable, Priority};
use temporalio_sdk::{
    ActivityCancellationType, ActivityOptions, WorkflowCancellationToken, WorkflowContext,
    WorkflowResult,
};
use temporalio_workflow::common::ActivityDefinition;

pub(crate) fn priority(priority: WorkPriority, fairness: String) -> Priority {
    let key = match priority {
        WorkPriority::NewActivity => 1,
        WorkPriority::Background => 5,
    };
    Priority::builder()
        .priority_key(key)
        .fairness_key(fairness)
        .build()
}

pub(crate) fn activity(priority: WorkPriority, fairness: String) -> ActivityOptions {
    ActivityOptions::with_start_to_close_timeout(policy::ACTIVITY_EXECUTION_TIMEOUT)
        .task_queue(crate::TASK_QUEUE)
        .priority(self::priority(priority, fairness))
        .do_not_eagerly_execute(true)
        .cancellation_type(ActivityCancellationType::WaitCancellationCompleted)
        .retry_policy(policy::activity_retry_policy())
        .build()
}

pub(super) fn manifest(priority: WorkPriority, fairness: String) -> ActivityOptions {
    let mut options = activity(priority, fairness);
    options.close_timeouts =
        temporalio_common::ActivityCloseTimeouts::StartToClose(policy::MANIFEST_EXECUTION_TIMEOUT);
    options.heartbeat_timeout = Some(policy::ACTIVITY_HEARTBEAT_TIMEOUT);
    options
}

pub(super) async fn run<W, A: ActivityDefinition>(
    ctx: &WorkflowContext<W>,
    definition: A,
    input: A::Input,
    options: ActivityOptions,
) -> WorkflowResult<A::Output>
where
    A::Input: Serialize,
    A::Output: TemporalDeserializable,
{
    workflow_result(ctx, Ok(()))?;
    let result = execute(ctx, definition, input, options).await;
    workflow_result(ctx, result)
}

async fn execute<W, A: ActivityDefinition>(
    ctx: &WorkflowContext<W>,
    definition: A,
    input: A::Input,
    options: ActivityOptions,
) -> WorkflowResult<A::Output>
where
    A::Input: Serialize,
    A::Output: TemporalDeserializable,
{
    ctx.execute_activity(definition, input, options)
        .await
        .map_err(|error| {
            if error.as_cancelled().is_some() {
                temporalio_sdk::WorkflowTermination::cancelled()
            } else {
                error.into()
            }
        })
}

/// Once source application starts, complete its small durable handoffs. A later
/// cancellation request is too late for this item; the workflow reports completion.
/// Long output calculations have their own independent workflows.
pub(crate) struct CommitBoundary<'a, W> {
    ctx: &'a WorkflowContext<W>,
}

impl<'a, W> CommitBoundary<'a, W> {
    pub fn begin(ctx: &'a WorkflowContext<W>) -> WorkflowResult<Self> {
        workflow_result(ctx, Ok(()))?;
        Ok(Self { ctx })
    }

    pub async fn run<A: ActivityDefinition>(
        &self,
        definition: A,
        input: A::Input,
        mut options: ActivityOptions,
    ) -> WorkflowResult<A::Output>
    where
        A::Input: Serialize,
        A::Output: TemporalDeserializable,
    {
        options.cancellation_token = Some(WorkflowCancellationToken::new());
        execute(self.ctx, definition, input, options).await
    }
}

/// A dependency may finish successfully after cancellation was requested.
/// Wait for it to settle, then preserve the parent's cancellation outcome.
pub(super) fn workflow_result<W, T>(
    ctx: &WorkflowContext<W>,
    result: WorkflowResult<T>,
) -> WorkflowResult<T> {
    if ctx.cancellation_token().is_cancelled() {
        Err(temporalio_sdk::WorkflowTermination::cancelled())
    } else {
        result
    }
}
