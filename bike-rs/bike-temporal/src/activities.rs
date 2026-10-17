//! Domain adapters are grouped by effect ownership. None may enqueue SQL tasks.
mod archive;
mod derived;
mod imports;
mod segments;
mod training;
pub use archive::{ArchiveActivities, ArchiveOperations};
pub use derived::{DerivedActivities, DerivedOperations};
pub use imports::{ImportActivities, ImportOperations, ReprocessManifestActivities, ReprocessManifestOperations};
pub use segments::{SegmentActivities, SegmentOperations};
pub use training::{TrainingActivities, TrainingOperations};

pub(crate) fn activity_error(
    error: impl std::error::Error + Send + Sync + 'static,
) -> temporalio_sdk::activities::ActivityError {
    temporalio_sdk::ApplicationFailure::new(error).into()
}

pub(crate) fn domain_error(
    error: bike_core::workflow_error::WorkflowError,
) -> temporalio_sdk::activities::ActivityError {
    let permanent = error.status.is_client_error();
    if permanent {
        temporalio_sdk::ApplicationFailure::non_retryable(error).into()
    } else {
        temporalio_sdk::ApplicationFailure::new(error).into()
    }
}

#[cfg(test)]
mod tests {
    use super::domain_error;
    use bike_core::workflow_error::WorkflowError;
    use temporalio_sdk::activities::ActivityError;

    #[test]
    fn immutable_input_failures_are_terminal_and_backend_failures_retry() {
        for (error, terminal) in [
            (WorkflowError::invalid_data("checksum or size mismatch"), true),
            (WorkflowError::bad_request("malformed retained file"), true),
            (WorkflowError::not_found("retained file missing"), true),
            (WorkflowError::internal("temporary storage failure"), false),
        ] {
            let ActivityError::Application(failure) = domain_error(error) else {
                panic!("Expected a native application failure");
            };
            assert_eq!(failure.is_non_retryable(), terminal);
        }
    }
}
