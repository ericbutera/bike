use super::fixtures::{assert_retryable, environment, Operations};
use crate::activities::ImportActivities;
use bike_core::{bike_tasks::*, test_support as fixtures};
use std::sync::Arc;

#[tokio::test]
async fn source_handler_returns_saved_identity_and_calls_the_domain() {
    let operations = Arc::new(Operations::default());
    let command = fixtures::import_command();
    let result = environment(operations.clone())
        .run(ImportActivities::activity_import, command.clone())
        .await
        .unwrap();
    assert_eq!(result, fixtures::import_result());
    assert_eq!(
        *operations.received.lock().unwrap(),
        vec![("activity_import", serde_json::to_value(command).unwrap())]
    );
}

#[tokio::test]
async fn transient_source_failure_remains_retryable() {
    let operations = Arc::new(Operations {
        fail: true,
        ..Default::default()
    });
    let error = environment(operations)
        .run(ImportActivities::activity_import, fixtures::import_command())
        .await
        .unwrap_err();
    assert_retryable(&error);
}

#[test]
fn reprocess_result_preserves_owner_activity_and_import() {
    let command = fixtures::reprocess_input().into_command();
    command.validate_result(&fixtures::import_result()).unwrap();
    for result in [
        ActivityImportResult {
            activity_id: 43,
            ..fixtures::import_result()
        },
        ActivityImportResult {
            user_id: 8,
            ..fixtures::import_result()
        },
        ActivityImportResult {
            activity_import_id: 19,
            ..fixtures::import_result()
        },
    ] {
        assert!(command.validate_result(&result).is_err());
    }
}
