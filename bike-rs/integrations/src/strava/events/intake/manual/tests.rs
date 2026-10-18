use super::*;
use bike_core::test_support as fixtures;
use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};
use std::sync::{Arc, Mutex};

struct Fixture {
    intake: StravaIntake,
    starts: Arc<Mutex<fixtures::temporal_client::Requests>>,
}

impl Fixture {
    async fn new(event: &StravaEvent) -> Self {
        let row = fixtures::strava_event_record(event);
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_exec_results([MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }])
            .append_query_results([vec![row.clone()]])
            .append_query_results([vec![fixtures::integration_event()]])
            .append_query_results([vec![row]])
            .into_connection();
        let starts = Default::default();
        let client = fixtures::temporal_client::client(Arc::clone(&starts)).await;
        Self {
            intake: StravaIntake::with_client(db, client),
            starts,
        }
    }

    async fn submit(&self, request: ManualRequest) -> Result<JobReference, WorkflowError> {
        request_manual(&self.intake, &fixtures::customer(), &fixtures::connection(), request).await
    }
}

fn request(operation: ManualOperation) -> ManualRequest {
    ManualRequest {
        command_id: Uuid::from_u128(65),
        request_id: Some("manual-admin".into()),
        operation,
    }
}

#[tokio::test]
async fn each_manual_operation_uses_native_submission_and_preserves_provider_history() {
    for (operation, workflow) in [
        (ManualOperation::Sync, "SyncActivities"),
        (
            ManualOperation::Backfill {
                after: fixtures::timestamp(),
                before: fixtures::timestamp() + chrono::Duration::days(1),
            },
            "BackfillActivities",
        ),
        (
            ManualOperation::Resync {
                bike_activity_id: 42,
                strava_activity_id: 999,
            },
            "UpsertActivity",
        ),
    ] {
        let request = request(operation);
        let event = request.event(&fixtures::customer(), &fixtures::connection()).unwrap();
        let fixture = Fixture::new(&event).await;
        let job = fixture.submit(request).await.unwrap();
        assert_eq!(job.workflow_type, workflow);
        assert_eq!(job.job_id, event.workflow_id());
        assert_eq!(job.run_id, "fixture-run");
        let starts = fixture.starts.lock().unwrap();
        let start = &starts.starts[0];
        assert_eq!(start.task_queue.as_ref().unwrap().name, "bike-strava");
        let input: StravaEvent = serde_json::from_slice(&start.input.as_ref().unwrap().payloads[0].data).unwrap();
        assert_eq!(input.event_time, 0);
        assert_eq!(input.request_id.as_deref(), Some("manual-admin"));
        assert_eq!(input.bike_user_id, fixtures::customer().id);
        assert_eq!(input.connection_generation, fixtures::connection().created_at);
        if workflow == "BackfillActivities" {
            let window = input.window.unwrap();
            assert_eq!(window.after, fixtures::timestamp().timestamp() - 1);
            assert_eq!(
                window.before,
                Some((fixtures::timestamp() + chrono::Duration::days(1)).timestamp())
            );
        }
        if workflow == "UpsertActivity" {
            assert_eq!(input.bike_activity_id, Some(42));
            assert_eq!(input.strava_activity_id, Some(999));
        }
        let log = format!("{:?}", fixture.intake.db.into_transaction_log());
        assert!(log.contains("strava.event.accepted"));
        assert!(log.contains("submitted_at"));
    }
}

#[test]
fn retries_keep_identity_while_different_targets_parameters_or_connections_do_not() {
    let user = fixtures::customer();
    let connection = fixtures::connection();
    let first = request(ManualOperation::Sync).event(&user, &connection).unwrap();
    let retry = ManualRequest {
        request_id: Some("new-http-request".into()),
        ..request(ManualOperation::Sync)
    };
    assert_eq!(
        first.workflow_id(),
        retry.event(&user, &connection).unwrap().workflow_id()
    );
    let changed = request(ManualOperation::Resync {
        bike_activity_id: 42,
        strava_activity_id: 999,
    });
    assert_ne!(
        first.workflow_id(),
        changed.event(&user, &connection).unwrap().workflow_id()
    );
    let reconnected = strava_connections::Model {
        created_at: connection.created_at + chrono::Duration::seconds(1),
        ..connection
    };
    assert_ne!(
        first.workflow_id(),
        retry.event(&user, &reconnected).unwrap().workflow_id()
    );
}

#[test]
fn validation_rejects_invalid_windows_missing_scopes_and_wrong_owners() {
    let user = fixtures::customer();
    let connection = fixtures::connection();
    assert!(request(ManualOperation::Backfill {
        after: fixtures::timestamp(),
        before: fixtures::timestamp()
    })
    .event(&user, &connection)
    .is_err());
    assert!(request(ManualOperation::Resync {
        bike_activity_id: 0,
        strava_activity_id: 999
    })
    .event(&user, &connection)
    .is_err());
    let wrong_owner = strava_connections::Model {
        user_id: user.id + 1,
        ..connection.clone()
    };
    assert!(request(ManualOperation::Sync).event(&user, &wrong_owner).is_err());
    let missing_scopes = strava_connections::Model {
        scopes: "read".into(),
        ..connection
    };
    assert!(request(ManualOperation::Sync).event(&user, &missing_scopes).is_err());
}

#[tokio::test]
async fn unavailable_temporal_leaves_the_real_provider_request_without_false_handoff() {
    let request = request(ManualOperation::Sync);
    let event = request.event(&fixtures::customer(), &fixtures::connection()).unwrap();
    let fixture = Fixture::new(&event).await;
    fixture.starts.lock().unwrap().fail_start = true;
    assert!(fixture.submit(request).await.unwrap_err().status.is_server_error());
    let log = format!("{:?}", fixture.intake.db.into_transaction_log());
    assert!(log.contains("strava.event.accepted"));
    assert!(!log.contains("UPDATE \"strava_events\""));
}
