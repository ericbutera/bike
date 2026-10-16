use super::fixtures::{assert_retryable, environment, MockImportAttempts, Operations};
use crate::activities::ImportActivities;
use bike_core::{bike_tasks::*, test_support as fixtures};
use std::sync::Arc;

#[tokio::test]
async fn import_records_domain_attempt_and_native_context_before_processing() {
    let operations = Arc::new(Operations::default());
    let execution = fixtures::import_execution();
    let result = environment(operations.clone())
        .run(ImportActivities::activity_import, execution.command.clone())
        .await
        .unwrap();
    assert_eq!(
        *operations.attempts.recorded.lock().unwrap(),
        vec![execution]
    );
    assert_eq!(
        *operations.attempts.completed.lock().unwrap(),
        vec![(fixtures::activity_import_attempt().id, result)],
    );
}

#[tokio::test]
async fn recording_failure_is_retryable_and_prevents_import_effects() {
    let operations = Arc::new(Operations {
        attempts: MockImportAttempts {
            fail_record: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let error = environment(operations.clone())
        .run(
            ImportActivities::activity_import,
            fixtures::import_command(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("fixture recording failed"));
    assert_retryable(&error);
    assert!(operations.received.lock().unwrap().is_empty());
    assert!(operations.attempts.completed.lock().unwrap().is_empty());
}

#[tokio::test]
async fn completion_recording_failure_is_a_retryable_activity_failure() {
    let operations = Arc::new(Operations {
        attempts: MockImportAttempts {
            fail_complete: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let command = fixtures::import_command();
    let error = environment(operations.clone())
        .run(ImportActivities::activity_import, command.clone())
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("fixture completion recording failed"));
    assert_retryable(&error);
    assert_eq!(
        *operations.received.lock().unwrap(),
        vec![("activity_import", serde_json::to_value(command).unwrap())],
    );
    assert_eq!(operations.attempts.completed.lock().unwrap().len(), 1);
}

#[test]
fn import_identity_is_stable_across_delivery_context_and_temporal_runs() {
    let execution = fixtures::import_execution();
    let mut retried = execution.clone();
    retried.workflow.run_id = "next-run".into();
    retried.command.source.cause.as_mut().unwrap().request_id = Some("retry".into());
    assert!(execution.matches_request(&retried));
    retried.command.source.activity_import_id += 1;
    assert!(!execution.matches_request(&retried));
}

#[test]
fn reprocess_result_must_keep_the_requested_activity_and_import_identity() {
    let command = ActivityImport {
        target: ImportTarget::ExistingActivity {
            activity_id: 42,
            expected_revision: 1,
        },
        ..fixtures::import_command()
    };
    assert!(command.validate_result(&fixtures::import_result()).is_ok());
    let result = ActivityImportResult {
        activity_id: 43,
        ..fixtures::import_result()
    };
    assert!(command.validate_result(&result).is_err());
}
