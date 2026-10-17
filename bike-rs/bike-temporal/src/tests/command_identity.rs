use super::fixtures::{reprocess_input, SubmissionFixture};
use bike_core::{bike_tasks::*, test_support as fixtures};

#[tokio::test]
async fn lost_reply_retry_uses_original_run_and_source_after_the_activity_revision_advances() {
    let fixture = SubmissionFixture::new().await;
    let accepted = fixture.reprocess_activity_import().await.unwrap();
    let retry = ReprocessInput {
        expected_revision: 999,
        ..reprocess_input()
    };
    let mut cause = fixtures::task_cause();
    cause.request_id = Some("retry-request".into());
    assert_eq!(
        fixture.service.reprocess_activity_import(retry, cause).await.unwrap(),
        accepted
    );
    let requests = fixture.requests.lock().unwrap();
    let original: ReprocessInput =
        serde_json::from_slice(&requests.starts[0].input.as_ref().unwrap().payloads[0].data).unwrap();
    assert_eq!(original.expected_revision, 3);
    assert_eq!(original.source.cause, Some(fixtures::task_cause()));
    assert_eq!(requests.histories.len(), 2);
}

#[tokio::test]
async fn retained_closed_command_returns_its_existing_run_without_restarting() {
    let fixture = SubmissionFixture::new().await;
    fixture.requests.lock().unwrap().already_started = true;
    let job = fixture.reprocess_activity_import().await.unwrap();
    assert_eq!(job.run_id, "fixture-run");
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.starts.len(), 1);
    assert_eq!(requests.histories.len(), 1);
}

#[tokio::test]
async fn duplicate_key_cannot_report_another_activity_import_or_object_as_accepted() {
    let fixture = SubmissionFixture::new().await;
    fixture.reprocess_activity_import().await.unwrap();
    let mut changed_activity = reprocess_input();
    changed_activity.activity_id += 1;
    let mut changed_import = reprocess_input();
    changed_import.source.activity_import_id += 1;
    let mut changed_object = reprocess_input();
    changed_object.source.object.path.push_str("-other");
    for input in [changed_activity, changed_import, changed_object] {
        let error = fixture
            .service
            .reprocess_activity_import(input, fixtures::task_cause())
            .await
            .unwrap_err();
        let error = error.downcast::<bike_core::workflow_error::WorkflowError>().unwrap();
        assert_eq!(error.status.as_u16(), 409);
    }
    let requests = fixture.requests.lock().unwrap();
    assert!(requests
        .starts
        .iter()
        .all(|request| request.workflow_id == requests.starts[0].workflow_id));
}

#[tokio::test]
async fn command_keys_are_scoped_to_the_affected_owner() {
    let fixture = SubmissionFixture::new().await;
    let first = fixture.reprocess_activity_import().await.unwrap();
    let mut input = reprocess_input();
    input.source.user_id += 1;
    let cause = TaskCause {
        user_id: input.source.user_id,
        ..fixtures::task_cause()
    };
    let second = fixture.service.reprocess_activity_import(input, cause).await.unwrap();
    assert_ne!(first.job_id, second.job_id);
}

#[tokio::test]
async fn unavailable_or_missing_retained_input_cannot_report_acceptance() {
    for missing in [false, true] {
        let fixture = SubmissionFixture::new().await;
        {
            let mut requests = fixture.requests.lock().unwrap();
            requests.fail_history = !missing;
            requests.missing_history = missing;
        }
        assert!(fixture.reprocess_activity_import().await.is_err());
    }
}
