use super::*;
use bike_core::{
    bike_tasks::{JobReference, ReprocessInput},
    test_support as fixtures,
};
use bike_temporal::submission::ReprocessSubmission;
use sea_orm::{DatabaseBackend, MockDatabase};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct Submission {
    received: Mutex<Vec<(ReprocessInput, TaskCause)>>,
    fail: bool,
}

#[async_trait::async_trait]
impl ReprocessSubmission for Submission {
    async fn reprocess_activity_import(
        &self,
        input: ReprocessInput,
        cause: TaskCause,
    ) -> Result<JobReference, bike_temporal::client::Error> {
        let job_id = format!(
            "bike-task-reprocess_activity_import-user-{}-{}",
            input.source.user_id, input.source.command_id
        );
        self.received.lock().unwrap().push((input, cause));
        if self.fail {
            return Err(std::io::Error::other("fixture Temporal unavailable").into());
        }
        Ok(JobReference {
            job_id,
            workflow_type: "reprocess_activity_import".into(),
            ..fixtures::job_reference()
        })
    }
}

struct Fixture {
    state: AppStorage,
    submission: Arc<Submission>,
    command_id: uuid::Uuid,
}

impl Fixture {
    fn new(fail: bool) -> Self {
        let mut mock = MockDatabase::new(DatabaseBackend::Postgres);
        // Two metadata-only submissions exercise the retry case using shared fixtures.
        for _ in 0..2 {
            mock = mock
                .append_query_results([vec![fixtures::reprocess_activity()]])
                .append_query_results([vec![fixtures::reprocess_import()]])
                .append_query_results([vec![fixtures::reprocess_artifact()]]);
        }
        let db = mock.into_connection();
        let submission = Arc::new(Submission {
            fail,
            ..Default::default()
        });
        let mut state = AppStorage::for_test(db);
        state.task_submission = submission.clone();
        Self {
            state,
            submission,
            command_id: uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000065").unwrap(),
        }
    }

    async fn submit(&self) -> Result<AcceptedReprocessing, AppError> {
        submit(
            &self.state,
            99,
            Some("request-fixture".into()),
            ReprocessActivityCommand {
                activity_id: fixtures::activity().id,
                command_id: self.command_id,
            },
        )
        .await
    }
}

#[tokio::test]
async fn native_acceptance_returns_job_without_source_bytes_or_a_worker() {
    let fixture = Fixture::new(false);
    let response = fixture.submit().await.unwrap();
    assert_eq!(response.activity_id, fixtures::activity().id);
    assert_eq!(response.activity_import_id, fixtures::activity_import().id);
    assert_eq!(response.user_id, fixtures::activity().user_id);
    let received = fixture.submission.received.lock().unwrap();
    assert_eq!(received.len(), 1);
    let (input, cause) = &received[0];
    assert_eq!(cause.actor, TaskActor::User(99));
    assert_eq!(cause.user_id, fixtures::activity().user_id);
    assert_eq!(cause.correlation_id, fixture.command_id.to_string());
    assert_eq!(cause.request_id.as_deref(), Some("request-fixture"));
    assert_eq!(input.activity_id, fixtures::activity().id);
    assert_eq!(input.source.priority, Priority::NewActivity);
}

#[tokio::test]
async fn retry_preserves_command_identity_and_returns_the_same_job() {
    let fixture = Fixture::new(false);
    assert_eq!(
        fixture.submit().await.unwrap().job,
        fixture.submit().await.unwrap().job
    );
    let received = fixture.submission.received.lock().unwrap();
    assert_eq!(received[0].0, received[1].0);
}

#[tokio::test]
async fn native_rejection_fails_the_api_without_sql_admission_or_fallback() {
    let fixture = Fixture::new(true);
    assert!(fixture
        .submit()
        .await
        .unwrap_err()
        .message
        .contains("fixture Temporal unavailable"));
    assert_eq!(fixture.submission.received.lock().unwrap().len(), 1);
}
