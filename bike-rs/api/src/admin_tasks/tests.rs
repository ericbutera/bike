use super::*;
use bike_core::test_support as fixtures;
use sea_orm::{DatabaseBackend, MockDatabase};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

struct Fixture {
    state: AppStorage,
    requests: Arc<Mutex<fixtures::temporal_client::Requests>>,
}

impl Fixture {
    async fn new(mock: MockDatabase) -> Self {
        let db = mock.into_connection();
        let requests = Default::default();
        let client = fixtures::temporal_client::client(Arc::clone(&requests)).await;
        let mut state = AppStorage::for_test(db.clone());
        state.tasks = bike_core::jobs::JobQueue::with_client(db.clone(), client.clone());
        state.task_submission = Arc::new(bike_temporal::submission::TaskSubmissionService::new(client.clone()));
        state.strava_intake = integrations::strava::events::StravaIntake::with_client(db, client);
        Self { state, requests }
    }

    async fn activity(&self, task: ActivityAction) -> Result<AcceptedTask, AppError> {
        activity(
            &self.state,
            context(),
            fixtures::activity().id,
            ActivityTaskRequest {
                command_id: command_id(),
                task,
            },
        )
        .await
    }

    async fn user(&self, action: UserAction) -> Result<AcceptedTask, AppError> {
        user(
            &self.state,
            context(),
            fixtures::customer().id,
            UserTaskRequest {
                command_id: command_id(),
                action,
            },
        )
        .await
    }

    fn history(&self) -> String {
        format!("{:?}", self.state.db.clone().into_transaction_log())
    }

    fn assert_start(&self, response: &AcceptedTask, workflow: &str) -> Value {
        assert_eq!(response.status, "accepted");
        assert_eq!(response.job.workflow_type, workflow);
        assert_eq!(response.job.namespace, "bike");
        assert_eq!(response.job.run_id, "fixture-run");
        let requests = self.requests.lock().unwrap();
        let start = requests.starts.last().unwrap();
        assert_eq!(start.task_queue.as_ref().unwrap().name, "bike-tasks");
        assert_eq!(start.workflow_type.as_ref().unwrap().name, workflow);
        assert_eq!(start.workflow_id, response.job.job_id);
        serde_json::from_slice(&start.input.as_ref().unwrap().payloads[0].data).unwrap()
    }
}

fn command_id() -> Uuid {
    Uuid::from_u128(65)
}

fn context() -> RequestContext {
    RequestContext {
        actor_id: 99,
        request_id: Some("admin-request".into()),
    }
}

fn mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
}

fn provider_acceptance(mock: MockDatabase, event: &integrations::strava::events::StravaEvent) -> MockDatabase {
    let row = fixtures::strava_event_record(event);
    mock.append_exec_results([sea_orm::MockExecResult {
        rows_affected: 1,
        last_insert_id: 0,
    }])
    .append_query_results([vec![row.clone()]])
    .append_query_results([vec![fixtures::integration_event()]])
    .append_query_results([vec![row]])
    .append_query_results([vec![fixtures::integration_event()]])
}

#[tokio::test]
async fn user_strava_actions_reuse_the_connected_accounts_intake_and_history() {
    use integrations::strava::events::{StravaEvent, StravaOperation};
    for (action, operation, workflow) in [
        (UserAction::StravaSync, StravaOperation::Sync, "SyncActivities"),
        (
            UserAction::StravaBackfill {
                after: fixtures::timestamp(),
                before: fixtures::timestamp() + chrono::Duration::days(1),
            },
            StravaOperation::Backfill,
            "BackfillActivities",
        ),
    ] {
        let event = StravaEvent::new(&fixtures::customer(), &fixtures::connection(), operation);
        let db = mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::connection()]]);
        let fixture = Fixture::new(provider_acceptance(db, &event)).await;
        let response = fixture.user(action).await.unwrap();
        assert_eq!(response.job.workflow_type, workflow);
        assert_eq!(response.job.run_id, "fixture-run");
        let log = fixture.history();
        assert!(log.contains("admin.task.accepted"));
        assert!(log.contains("strava.event.accepted"));
    }
}

#[tokio::test]
async fn activity_strava_resync_resolves_the_current_connection_mapping_before_intake() {
    use integrations::strava::events::{StravaEvent, StravaOperation};
    let mapping = fixtures::strava_activity_state();
    let mut event = StravaEvent::new(&fixtures::customer(), &fixtures::connection(), StravaOperation::Upsert);
    event.strava_activity_id = Some(mapping.activity_id);
    event.bike_activity_id = Some(fixtures::activity().id);
    let db = mock()
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![fixtures::customer()]])
        .append_query_results([vec![fixtures::connection()]])
        .append_query_results([vec![mapping]]);
    let fixture = Fixture::new(provider_acceptance(db, &event)).await;
    let response = fixture.activity(ActivityAction::StravaResync).await.unwrap();
    assert_eq!(response.job.workflow_type, "UpsertActivity");
    assert_eq!(response.activity_id, Some(fixtures::activity().id));
    let requests = fixture.requests.lock().unwrap();
    let input: Value = serde_json::from_slice(&requests.starts[0].input.as_ref().unwrap().payloads[0].data).unwrap();
    assert_eq!(input["strava_activity_id"], event.strava_activity_id.unwrap());
    assert!(fixture.history().contains("admin.task.accepted"));
}

#[tokio::test]
async fn activity_actions_use_the_stored_owner_and_record_the_acknowledged_job() {
    for (action, workflow) in [
        (ActivityAction::RebuildFitness, "rebuild_fitness_freshness"),
        (ActivityAction::RebuildSegments, "regenerate_activity_segments"),
        (ActivityAction::RebuildHeatmap, "prepare_heatmap"),
    ] {
        let fixture = Fixture::new(
            mock()
                .append_query_results([vec![fixtures::activity()]])
                .append_query_results([vec![fixtures::integration_event()]]),
        )
        .await;
        let response = fixture.activity(action).await.unwrap();
        let input = fixture.assert_start(&response, workflow);
        assert_eq!(input["user_id"], fixtures::activity().user_id);
        if !matches!(action, ActivityAction::RebuildFitness) {
            assert_eq!(input["activity_id"], fixtures::activity().id);
        }
        assert_eq!(response.activity_id, Some(fixtures::activity().id));
        let history = fixture.history();
        for expected in [
            "admin.task.accepted",
            "actor_user_id",
            "99",
            "admin-request",
            &response.job.job_id,
        ] {
            assert!(history.contains(expected), "missing {expected}: {history}");
        }
    }
}

#[tokio::test]
async fn user_actions_preserve_each_existing_workflow_and_first_page() {
    for (action, workflow) in [
        (UserAction::RebuildFitness, "rebuild_fitness_freshness"),
        (UserAction::RebuildSegments, "regenerate_user_segments"),
        (
            UserAction::ReprocessImports {
                archive_fits_only: false,
            },
            "reprocess_user_activity_imports",
        ),
        (
            UserAction::ReprocessImports {
                archive_fits_only: true,
            },
            "reprocess_archive_fit_activity_imports",
        ),
        (UserAction::BackfillXcTraining, "backfill_user_xc_training"),
    ] {
        let db = mock().append_query_results([vec![fixtures::customer()]]);
        let fixture = Fixture::new(db.append_query_results([vec![fixtures::integration_event()]])).await;
        let response = fixture.user(action).await.unwrap();
        let input = fixture.assert_start(&response, workflow);
        assert_eq!(input["user_id"], fixtures::customer().id);
        assert_eq!(response.activity_id, None);
        if workflow != "rebuild_fitness_freshness" {
            assert_eq!(input["after_id"], 0);
        }
    }
}

#[tokio::test]
async fn reprocess_uses_the_existing_source_metadata_submission() {
    let fixture = Fixture::new(
        mock()
            .append_query_results([
                vec![fixtures::reprocess_activity()],
                vec![fixtures::reprocess_activity()],
            ])
            .append_query_results([vec![fixtures::reprocess_import()]])
            .append_query_results([vec![fixtures::reprocess_artifact()]])
            .append_query_results([vec![fixtures::integration_event()]]),
    )
    .await;
    let response = fixture.activity(ActivityAction::Reprocess).await.unwrap();
    fixture.assert_start(&response, "reprocess_activity_import");
    assert!(fixture.history().contains("admin.task.accepted"));
}

#[tokio::test]
async fn retry_after_uncertain_start_returns_the_same_job_and_policies_prevent_restart() {
    let mut db = mock();
    for _ in 0..2 {
        db = db
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::integration_event()]]);
    }
    let fixture = Fixture::new(db).await;
    let first = fixture.user(UserAction::RebuildSegments).await.unwrap();
    let second = fixture.user(UserAction::RebuildSegments).await.unwrap();
    assert_eq!(first.job, second.job);
    let requests = fixture.requests.lock().unwrap();
    let start = &requests.starts[0];
    assert_eq!(
        start.workflow_id_conflict_policy().as_str_name(),
        "WORKFLOW_ID_CONFLICT_POLICY_USE_EXISTING"
    );
    assert_eq!(
        start.workflow_id_reuse_policy().as_str_name(),
        "WORKFLOW_ID_REUSE_POLICY_REJECT_DUPLICATE"
    );
    assert!(start.retry_policy.is_none());
}

#[tokio::test]
async fn rejected_start_records_failure_without_claiming_acceptance() {
    let fixture = Fixture::new(
        mock()
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::integration_event()]]),
    )
    .await;
    fixture.requests.lock().unwrap().fail_start = true;
    assert!(fixture
        .user(UserAction::RebuildSegments)
        .await
        .unwrap_err()
        .status
        .is_server_error());
    let history = fixture.history();
    assert!(history.contains("admin.task.submission_failed"));
    assert!(!history.contains("admin.task.accepted"));
}

#[tokio::test]
async fn history_write_failure_can_be_retried_without_starting_a_second_workflow() {
    let db = mock()
        .append_query_results([vec![fixtures::customer()]])
        .append_query_errors([sea_orm::DbErr::Custom("history write failed".into())])
        .append_query_results([vec![fixtures::customer()]])
        .append_query_results([vec![fixtures::integration_event()]]);
    let fixture = Fixture::new(db).await;
    assert!(fixture.user(UserAction::BackfillXcTraining).await.is_err());
    let accepted = fixture.user(UserAction::BackfillXcTraining).await.unwrap();
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.starts.len(), 2);
    assert_eq!(requests.starts[0].workflow_id, requests.starts[1].workflow_id);
    assert_eq!(accepted.job.job_id, requests.starts[0].workflow_id);
    // Retrying a finished job must not reset the XC page's completion state.
    assert!(!fixture.history().contains("UPDATE \"user_preferences\""));
}

#[tokio::test]
async fn missing_target_and_missing_strava_mapping_never_start_work() {
    let fixture = Fixture::new(mock().append_query_results([Vec::<activities::Model>::new()])).await;
    assert_eq!(
        fixture
            .activity(ActivityAction::StravaResync)
            .await
            .unwrap_err()
            .status
            .as_u16(),
        404
    );
    assert!(fixture.requests.lock().unwrap().starts.is_empty());

    let fixture = Fixture::new(
        mock()
            .append_query_results([vec![fixtures::activity()]])
            .append_query_results([vec![fixtures::customer()]])
            .append_query_results([vec![fixtures::connection()]])
            .append_query_results([Vec::<bike_core::entities::strava_activity_states::Model>::new()])
            .append_query_results([vec![fixtures::integration_event()]]),
    )
    .await;
    assert_eq!(
        fixture
            .activity(ActivityAction::StravaResync)
            .await
            .unwrap_err()
            .status
            .as_u16(),
        400
    );
    assert!(fixture.requests.lock().unwrap().starts.is_empty());
    let history = fixture.history();
    for filter in ["connection_generation", "athlete_id", "bike_activity_id", "deleted"] {
        assert!(history.contains(filter));
    }
}

#[test]
fn typed_parameters_reject_unknown_tasks_extra_owner_ids_and_missing_dates() {
    let command = command_id();
    for value in [
        json!({"command_id": command, "task": "arbitrary_workflow"}),
        json!({"command_id": command, "task": "rebuild_fitness", "user_id": 456}),
    ] {
        assert!(serde_json::from_value::<ActivityTaskRequest>(value).is_err());
    }
    assert!(serde_json::from_value::<UserTaskRequest>(json!({
        "command_id": command, "task": "strava_backfill", "after": fixtures::timestamp(),
    }))
    .is_err());
    assert!(serde_json::from_value::<UserTaskRequest>(json!({
        "command_id": command, "task": "reprocess_imports", "user_id": 456,
    }))
    .is_err());
    let request: UserTaskRequest = serde_json::from_value(json!({
        "command_id": command, "task": "reprocess_imports", "archive_fits_only": true,
    }))
    .unwrap();
    assert!(matches!(
        request.action,
        UserAction::ReprocessImports {
            archive_fits_only: true
        }
    ));
}
