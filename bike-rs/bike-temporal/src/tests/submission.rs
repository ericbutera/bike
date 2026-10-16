use super::fixtures::{reprocess_input, SubmissionFixture};
use bike_core::{bike_tasks::*, test_support as fixtures};

#[tokio::test]
async fn submission_returns_native_reference_and_preserves_causal_input() {
    let fixture = SubmissionFixture::new().await;
    let job = fixture.reprocess_activity_import().await.unwrap();
    assert_eq!(job.run_id, "fixture-run");
    assert_eq!(job.namespace, "bike");
    assert_eq!(job.workflow_type, "reprocess_activity_import");
    let requests = fixture.requests.lock().unwrap();
    let start = &requests.starts[0];
    assert_eq!(job.job_id, start.workflow_id);
    let submitted: ReprocessInput =
        serde_json::from_slice(&start.input.as_ref().unwrap().payloads[0].data).unwrap();
    assert_eq!(submitted.source.cause, Some(fixtures::task_cause()));
}

#[tokio::test]
async fn both_import_entry_points_preserve_the_same_context() {
    let fixture = SubmissionFixture::new().await;
    fixture.process_activity_import().await.unwrap();
    fixture.reprocess_activity_import().await.unwrap();
    let requests = fixture.requests.lock().unwrap();
    let input: ImportSource =
        serde_json::from_slice(&requests.starts[0].input.as_ref().unwrap().payloads[0].data)
            .unwrap();
    assert_eq!(input.cause, Some(fixtures::task_cause()));
}

#[tokio::test]
async fn invalid_commands_and_context_fail_before_temporal_submission() {
    let fixture = SubmissionFixture::new().await;
    for cause in [
        TaskCause {
            correlation_id: " ".into(),
            ..fixtures::task_cause()
        },
        TaskCause {
            actor: TaskActor::System(" ".into()),
            ..fixtures::task_cause()
        },
        TaskCause {
            user_id: 99,
            ..fixtures::task_cause()
        },
    ] {
        assert!(fixture
            .service
            .reprocess_activity_import(reprocess_input(), cause)
            .await
            .is_err());
    }
    let invalid = ReprocessInput {
        expected_revision: 0,
        ..reprocess_input()
    };
    assert!(fixture
        .service
        .reprocess_activity_import(invalid, fixtures::task_cause())
        .await
        .is_err());
    assert!(fixture.requests.lock().unwrap().starts.is_empty());
}

#[tokio::test]
async fn invalid_command_is_rejected_before_the_lazy_connection() {
    let service = crate::submission::TaskSubmissionService::from_env();
    let input = ReprocessInput {
        expected_revision: 0,
        ..reprocess_input()
    };
    let expected = input
        .clone()
        .into_command()
        .validate()
        .unwrap_err()
        .to_string();
    let error = service
        .reprocess_activity_import(input, fixtures::task_cause())
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), expected);
}

#[tokio::test]
async fn temporal_rejection_returns_an_error_without_api_side_compensation() {
    let fixture = SubmissionFixture::new().await;
    fixture.requests.lock().unwrap().fail_start = true;
    assert!(fixture.process_activity_import().await.is_err());
    assert_eq!(fixture.requests.lock().unwrap().starts.len(), 1);
}

#[tokio::test]
async fn duplicate_completed_command_returns_temporals_existing_reference() {
    let fixture = SubmissionFixture::new().await;
    fixture.requests.lock().unwrap().already_started = true;
    let job = fixture.process_activity_import().await.unwrap();
    assert_eq!(job.run_id, "fixture-run");
    assert_eq!(fixture.requests.lock().unwrap().starts.len(), 1);
}
