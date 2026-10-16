use bike_core::bike_tasks::Priority as WorkPriority;
use bike_core::platform::temporal as policy;
use serde::Serialize;
use temporalio_common::{data_converters::TemporalDeserializable, Priority};
use temporalio_sdk::{ActivityCancellationType, ActivityOptions, WorkflowContext, WorkflowResult};
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
    let result = ctx
        .execute_activity(definition, input, options)
        .await
        .map_err(|error| {
            if error.as_cancelled().is_some() {
                temporalio_sdk::WorkflowTermination::cancelled()
            } else {
                error.into()
            }
        });
    workflow_result(ctx, result)
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
