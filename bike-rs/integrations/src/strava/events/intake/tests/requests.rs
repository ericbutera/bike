use super::*;
use bike_core::test_support as fixture;
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, MockExecResult};

fn acceptance_mock(operation: StravaOperation, connection: strava_connections::Model) -> MockDatabase {
    let mut event = StravaEvent::new(&fixture::customer(), &connection, operation);
    match operation {
        StravaOperation::Backfill => {
            event.window = Some(crate::strava::native::ActivityWindow {
                after: fixture::timestamp().timestamp() - 1,
                before: Some((fixture::timestamp() + chrono::Duration::days(1)).timestamp()),
            })
        }
        StravaOperation::Disconnect => event.revoke_remote = true,
        _ => {}
    }
    let row = fixture::strava_event_record(&event);
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row.clone()]])
        .append_query_results([vec![fixture::integration_event()]])
        .append_query_results([vec![row]])
}

async fn request(
    db: &StravaIntake,
    operation: StravaOperation,
    connection: &strava_connections::Model,
) -> Result<strava_events::Model, WorkflowError> {
    let user = fixture::customer();
    match operation {
        StravaOperation::Sync => request_sync(db, &user, connection).await,
        StravaOperation::Backfill => {
            let after = fixture::timestamp();
            request_backfill(db, &user, connection, after, after + chrono::Duration::days(1)).await
        }
        StravaOperation::Disconnect => request_disconnect(db, &user, connection).await,
        _ => panic!("Expected a user-requested collection operation"),
    }
}

#[tokio::test]
async fn accepted_requests_use_the_loaded_connection_without_reading_it_again() {
    for operation in [
        StravaOperation::Sync,
        StravaOperation::Backfill,
        StravaOperation::Disconnect,
    ] {
        let connection = strava_connections::Model {
            last_sync_status: "succeeded".into(),
            last_sync_imported_count: 3,
            last_sync_duplicate_count: 2,
            ..fixture::connection()
        };
        let db = acceptance_mock(operation, connection.clone())
            .append_exec_results([MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }])
            .append_query_errors([DbErr::Custom("connection removed after acceptance".into())])
            .into_connection();
        let intake = Intake::new(&db).await;
        let accepted = request(&intake.service, operation, &connection).await.unwrap();
        assert_eq!(intake.requests.lock().unwrap().starts.len(), 1);
        let starts = intake.requests.lock().unwrap().starts.clone();
        let input: StravaEvent = serde_json::from_slice(&starts[0].input.as_ref().unwrap().payloads[0].data).unwrap();
        assert_eq!(input.operation, operation);
        assert_eq!(input.window.is_some(), operation == StravaOperation::Backfill);
        assert_eq!(input.revoke_remote, operation == StravaOperation::Disconnect);
        assert_eq!(accepted.user_id, connection.user_id);
        assert_eq!(accepted.athlete_id, connection.athlete_id);
        let log = db.into_transaction_log();
        let connection_reads = log
            .iter()
            .flat_map(|transaction| transaction.statements())
            .filter(|statement| statement.sql.contains("FROM \"strava_connections\""))
            .count();
        assert_eq!(connection_reads, 0);
        assert!(format!("{log:?}").contains("COMMIT"));
    }
}

#[tokio::test]
async fn failed_acceptance_does_not_acknowledge_the_request() {
    for operation in [
        StravaOperation::Sync,
        StravaOperation::Backfill,
        StravaOperation::Disconnect,
    ] {
        let db = acceptance_mock(operation, fixture::connection())
            .append_exec_errors([DbErr::Custom("event persistence unavailable".into())])
            .into_connection();
        let intake = Intake::new(&db).await;
        let error = request(&intake.service, operation, &fixture::connection())
            .await
            .unwrap_err();
        assert!(intake.requests.lock().unwrap().starts.is_empty());
        assert!(error.status.is_server_error());
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("ROLLBACK"));
        assert!(!log.contains("COMMIT"));
    }
}

#[tokio::test]
async fn disconnecting_connection_rejects_sync_and_backfill_before_acceptance() {
    for operation in [StravaOperation::Sync, StravaOperation::Backfill] {
        let connection = strava_connections::Model {
            disconnect_requested_at: Some(fixture::timestamp()),
            ..fixture::connection()
        };
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        assert!(request(&Intake::new(&db).await.service, operation, &connection)
            .await
            .unwrap_err()
            .status
            .is_client_error());
        assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT INTO"));
    }
}

#[tokio::test]
async fn a_connection_owned_by_another_user_is_rejected_before_acceptance() {
    for operation in [
        StravaOperation::Sync,
        StravaOperation::Backfill,
        StravaOperation::Disconnect,
    ] {
        let connection = strava_connections::Model {
            user_id: fixture::customer().id + 1,
            ..fixture::connection()
        };
        let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
        assert!(request(&Intake::new(&db).await.service, operation, &connection)
            .await
            .is_err());
        assert!(db.into_transaction_log().is_empty());
    }
}

#[tokio::test]
async fn requests_return_failure_when_temporal_rejects_the_start() {
    for operation in [
        StravaOperation::Sync,
        StravaOperation::Backfill,
        StravaOperation::Disconnect,
    ] {
        let connection = fixture::connection();
        let db = acceptance_mock(operation, connection.clone())
            .append_exec_results([MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }])
            .into_connection();
        let intake = Intake::new(&db).await;
        intake.requests.lock().unwrap().fail_start = true;
        let error = request(&intake.service, operation, &connection).await.unwrap_err();
        assert!(error.message.contains("fixture start refused"));
        assert_eq!(intake.requests.lock().unwrap().starts.len(), 1);
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("COMMIT"));
        assert!(!log.contains("UPDATE \"strava_events\""));
    }
}

#[tokio::test]
async fn redelivery_after_a_failed_start_submits_the_original_accepted_webhook() {
    let fixture = fixture::strava_fixture::Fixture::new();
    let mut original = fixture::strava_event_record(&fixture.event);
    original.event_key = fixture.event.webhook_key().unwrap();
    let db = fixture::postgres_mock()
        .append_exec_results((0..2).map(|_| MockExecResult {
            rows_affected: 0,
            last_insert_id: 0,
        }))
        .append_query_results([vec![original.clone()], vec![original.clone()], vec![original.clone()]])
        .into_connection();
    let intake = Intake::new(&db).await;
    let mut delivery = fixture.event.clone();
    delivery.strava_event_id = uuid::Uuid::new_v4();
    intake.requests.lock().unwrap().fail_start = true;
    assert!(intake.service.accept_event(delivery.clone()).await.is_err());
    intake.requests.lock().unwrap().fail_start = false;
    delivery.strava_event_id = uuid::Uuid::new_v4();
    let accepted = intake.service.accept_event(delivery).await.unwrap();
    assert_eq!(accepted.id, original.id);
    let requests = intake.requests.lock().unwrap();
    assert_eq!(requests.starts.len(), 2);
    for start in &requests.starts {
        assert_eq!(start.workflow_id, original.workflow_id);
        let input: StravaEvent = serde_json::from_slice(&start.input.as_ref().unwrap().payloads[0].data).unwrap();
        assert_eq!(input.strava_event_id, fixture.event.strava_event_id);
    }
}
