//! Named Activities adapt SDK calls to domain operations.
mod batches;
mod derived;
pub use batches::{BatchActivities, BatchOperations};
mod imports;
pub use derived::{DerivedActivities, DerivedOperations};
pub use imports::{ImportActivities, ImportOperations};

/// Domain work includes CPU-bound parsing and matching. Keep Temporal polling,
/// timers, and health requests responsive while those operations run.
pub(crate) async fn domain_task<T: Send + 'static>(
    operation: impl std::future::Future<Output = Result<T, bike_core::workflow_error::WorkflowError>> + Send + 'static,
) -> Result<T, bike_core::workflow_error::WorkflowError> {
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || runtime.block_on(operation))
        .await
        .map_err(|error| bike_core::workflow_error::WorkflowError::internal(format!("Domain task failed: {error}")))?
}

pub(crate) fn domain_error(
    error: bike_core::workflow_error::WorkflowError,
) -> temporalio_sdk::activities::ActivityError {
    if error.status.is_client_error() {
        temporalio_sdk::ApplicationFailure::non_retryable(error).into()
    } else {
        temporalio_sdk::ApplicationFailure::new(error).into()
    }
}

#[cfg(test)]
mod tests {
    use super::{domain_error, domain_task};
    use bike_core::workflow_error::WorkflowError;
    use temporalio_sdk::activities::ActivityError;

    #[tokio::test]
    async fn blocking_domain_work_leaves_the_async_worker_responsive() {
        let (started, ready) = tokio::sync::oneshot::channel();
        let (release, wait) = std::sync::mpsc::channel();
        let activity = tokio::spawn(domain_task(async move {
            started.send(()).unwrap();
            wait.recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|_| WorkflowError::internal("Worker event loop was blocked"))?;
            Ok(42)
        }));
        ready.await.unwrap();
        release.send(()).unwrap();
        assert_eq!(activity.await.unwrap().unwrap(), 42);
    }

    #[tokio::test]
    async fn blocking_domain_work_preserves_terminal_errors() {
        let error = domain_task(async { Err::<(), _>(WorkflowError::invalid_data("checksum mismatch")) })
            .await
            .unwrap_err();
        assert_eq!(error.status.as_u16(), 422);
    }

    #[test]
    fn invalid_input_is_terminal_and_backend_failures_retry() {
        for (error, terminal) in [
            (WorkflowError::invalid_data("checksum mismatch"), true),
            (WorkflowError::bad_request("malformed retained file"), true),
            (WorkflowError::not_found("retained file missing"), true),
            (WorkflowError::internal("temporary storage failure"), false),
        ] {
            let ActivityError::Application(failure) = domain_error(error) else {
                panic!("Expected native application failure");
            };
            assert_eq!(failure.is_non_retryable(), terminal);
        }
    }
}
