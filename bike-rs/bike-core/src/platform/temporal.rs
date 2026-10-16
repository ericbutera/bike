//! Shared native Activity policies for Bike and provider workflows.
//! Workflow executions have no automatic retry policy; retry individual effects.
use std::time::Duration;
use temporalio_common::RetryPolicy;

pub const ACTIVITY_EXECUTION_TIMEOUT: Duration = Duration::from_secs(120);
pub const ACTIVITY_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(30);
pub const ACTIVITY_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
pub const MANIFEST_EXECUTION_TIMEOUT: Duration = Duration::from_secs(3600);

pub const RETRY_INITIAL_INTERVAL: Duration = Duration::from_secs(1);
const RETRY_BACKOFF_COEFFICIENT: f64 = 2.0;
const ACTIVITY_MAXIMUM_ATTEMPTS: u32 = 12;
const ACTIVITY_MAXIMUM_RETRY_INTERVAL: Duration = Duration::from_secs(60);

/// Up to twelve total attempts for domain effects, with exponential backoff.
pub fn activity_retry_policy() -> RetryPolicy {
    RetryPolicy::builder()
        .initial_interval(RETRY_INITIAL_INTERVAL)
        .backoff_coefficient(RETRY_BACKOFF_COEFFICIENT)
        .maximum_attempts(ACTIVITY_MAXIMUM_ATTEMPTS)
        .maximum_interval(ACTIVITY_MAXIMUM_RETRY_INTERVAL)
        .build()
}
