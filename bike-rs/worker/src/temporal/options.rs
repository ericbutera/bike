use std::time::Duration;
use temporalio_common::RetryPolicy;
use temporalio_sdk::{ActivityCancellationType, ActivityOptions};

pub(super) fn termination(
    error: temporalio_common::error::ActivityExecutionError,
) -> temporalio_workflow::WorkflowTermination {
    if error.as_cancelled().is_some() {
        temporalio_workflow::WorkflowTermination::cancelled()
    } else {
        error.into()
    }
}

pub(super) fn step(summary: &str) -> ActivityOptions {
    ActivityOptions::with_start_to_close_timeout(Duration::from_secs(120))
        .heartbeat_timeout(Duration::from_secs(30))
        .cancellation_type(ActivityCancellationType::WaitCancellationCompleted)
        .retry_policy(
            RetryPolicy::builder()
                .maximum_attempts(12)
                .maximum_interval(Duration::from_secs(60))
                .build(),
        )
        .summary(summary.to_owned())
        .build()
}

pub(super) fn publication() -> ActivityOptions {
    let mut options = step("Wait for published Bike outputs");
    options.close_timeouts =
        temporalio_common::ActivityCloseTimeouts::StartToClose(Duration::from_secs(1800));
    options
}
