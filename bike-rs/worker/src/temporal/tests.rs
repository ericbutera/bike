use super::activities::activity_error;
use bike_core::workflow_error::WorkflowError;
use temporalio_sdk::activities::ActivityError;

#[test]
fn invalid_operation_is_terminal_and_provider_failure_is_retryable() {
    for (error, expected) in [
        (WorkflowError::bad_request("wrong operation"), true),
        (WorkflowError::internal("provider unavailable"), false),
    ] {
        let ActivityError::Application(failure) = activity_error(error) else {
            panic!("Expected application failure");
        };
        assert_eq!(failure.is_non_retryable(), expected);
    }
}

#[test]
fn quota_deferral_preserves_the_provider_retry_time() {
    let error = WorkflowError::too_many_requests(
        "quota exhausted",
        Some(chrono::Utc::now() + chrono::Duration::seconds(60)),
    );
    let ActivityError::Application(failure) = activity_error(error) else {
        panic!("Expected application failure");
    };
    assert!(!failure.is_non_retryable());
    assert!(failure.next_retry_delay().unwrap().as_secs() >= 59);
}
