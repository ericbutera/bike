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
    Priority::builder().priority_key(key).fairness_key(fairness).build()
}

pub(crate) fn activity(priority: WorkPriority, fairness: String) -> ActivityOptions {
    // Whole archives and segment rematching can outlive a provider RPC timeout.
    ActivityOptions::with_start_to_close_timeout(std::time::Duration::from_secs(30 * 60))
        .task_queue(crate::TASK_QUEUE)
        .priority(self::priority(priority, fairness))
        .do_not_eagerly_execute(true)
        .cancellation_type(ActivityCancellationType::WaitCancellationCompleted)
        .retry_policy(policy::activity_retry_policy())
        .build()
}

/// Keep SDK error adaptation here; the workflow shows the actual domain steps.
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
    if ctx.cancellation_token().is_cancelled() {
        return Err(temporalio_sdk::WorkflowTermination::cancelled());
    }
    ctx.execute_activity(definition, input, options).await.map_err(|error| {
        if error.as_cancelled().is_some() {
            temporalio_sdk::WorkflowTermination::cancelled()
        } else {
            error.into()
        }
    })
}
