use super::*;
use bike_core::test_support as fixture;
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, MockExecResult};

fn acceptance_mock(operation: StravaOperation, connection: strava_connections::Model) -> MockDatabase {
    let event = StravaEvent::new(&fixture::customer(), &connection, operation);
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture::strava_event_record(&event)]])
        .append_query_results([vec![fixture::integration_event()]])
}

async fn request(
    db: &DatabaseConnection,
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
        let accepted = request(&db, operation, &connection).await.unwrap();
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
        let error = request(&db, operation, &fixture::connection()).await.unwrap_err();
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
        assert!(request(&db, operation, &connection)
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
        assert!(request(&db, operation, &connection).await.is_err());
        assert!(db.into_transaction_log().is_empty());
    }
}
