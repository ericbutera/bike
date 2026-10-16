use super::*;
use crate::strava::{events::StravaOperation, native::strava_native_unit::fixture::audit};

const PREPARE: &str = "Prepare scheduled Strava syncs";
const VALIDATE: &str = "Validate Strava connection and customer";

fn sync_fixture() -> Fixture {
    let mut fixture = Fixture::new();
    fixture.event.operation = StravaOperation::Sync;
    fixture.event.strava_activity_id = None;
    fixture.event.strava_event_id = uuid::Uuid::new_v5(
        &fixture.event.correlation_id,
        &fixture.connection.id.to_be_bytes(),
    );
    fixture
}

fn request(fixture: &Fixture) -> ScanRequest {
    ScanRequest {
        correlation_id: fixture.event.correlation_id,
        after_connection_id: 0,
        failed_customers: 0,
    }
}

fn scan_mock(fixture: &Fixture, connections: Vec<strava_connections::Model>) -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([connections])
        .append_query_results([vec![fixture.customer.clone()]])
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.customer.clone()]])
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![audit(fixture)]])
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
}

fn assert_progress(span: &SpanData, scanned: i64, prepared: i64) {
    for (key, value) in [
        ("connections_scanned", scanned),
        ("sync_events_prepared", prepared),
    ] {
        let field = span
            .attributes
            .iter()
            .find(|field| field.key.as_str() == key);
        assert_eq!(
            field.map(|field| &field.value),
            Some(&value.into()),
            "{key}"
        );
    }
}

#[tokio::test]
async fn scheduled_sync_setup_groups_reads_acceptance_and_submission_under_one_action() {
    let fixture = sync_fixture();
    let skipped = strava_connections::Model {
        id: fixture.connection.id + 1,
        scopes: "read".into(),
        ..fixture.connection.clone()
    };
    let operations = StravaOperations::new(
        scan_mock(&fixture, vec![fixture.connection.clone(), skipped]).into_connection(),
    );
    let trace = capture(async {
        let page = operations
            .scan_connections(request(&fixture))
            .await
            .unwrap();
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.events[0].operation, StravaOperation::Sync);
        assert_eq!(
            page.events[0].strava_event_id,
            fixture.event.strava_event_id
        );
        assert_eq!(page.next_connection_id, None);
    })
    .await;
    let preparation = stage(&trace, PREPARE);
    assert_progress(&preparation, 2, 1);
    let validation = trace
        .spans()
        .into_iter()
        .find(|span| span.name == VALIDATE)
        .unwrap();
    assert_eq!(
        validation.parent_span_id,
        preparation.span_context.span_id()
    );
    let transactions = operations.db.into_transaction_log();
    assert_eq!(transactions.len(), 6);
    let acceptance = transactions[4].statements();
    assert!(acceptance.iter().any(|statement| statement
        .sql
        .starts_with("INSERT INTO \"integration_events\"")));
    assert_eq!(acceptance.last().unwrap().sql, "COMMIT");
    assert!(transactions[5].statements()[0]
        .sql
        .starts_with("UPDATE \"strava_events\""));
}

#[tokio::test]
async fn empty_scan_explains_its_database_work_without_customer_spans() {
    let fixture = sync_fixture();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<strava_connections::Model>::new()])
        .into_connection();
    let operations = StravaOperations::new(db);
    let trace = capture(async {
        assert!(operations
            .scan_connections(request(&fixture))
            .await
            .unwrap()
            .events
            .is_empty());
    })
    .await;
    assert_progress(&stage(&trace, PREPARE), 0, 0);
    assert!(!trace.spans().iter().any(|span| span.name == VALIDATE));
}

#[tokio::test]
async fn failed_scan_keeps_its_query_parent_without_reporting_successful_preparation() {
    let fixture = sync_fixture();
    let next = strava_connections::Model {
        id: fixture.connection.id + 1,
        ..fixture.connection.clone()
    };
    let db = scan_mock(&fixture, vec![fixture.connection.clone(), next])
        .append_query_errors([sea_orm::DbErr::Custom(
            "fixture customer read failed".into(),
        )])
        .into_connection();
    let operations = StravaOperations::new(db);
    let trace = capture(async {
        assert!(operations
            .scan_connections(request(&fixture))
            .await
            .is_err());
    })
    .await;
    let preparation = stage(&trace, PREPARE);
    assert!(preparation.attributes.iter().any(|field| {
        field.key.as_str() == "connections_scanned" && field.value == 2_i64.into()
    }));
    assert!(!preparation
        .attributes
        .iter()
        .any(|field| field.key.as_str() == "sync_events_prepared"));
}

#[tokio::test]
async fn connection_validation_groups_both_ownership_reads_and_their_duration() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.customer.clone()]])
        .into_connection();
    let operations = StravaOperations::new(db);
    let trace = capture(async {
        let connection = operations.connection(&fixture.event).await.unwrap();
        assert_eq!(connection.id, fixture.connection.id);
    })
    .await;
    let validation = stage(&trace, VALIDATE);
    assert!(validation.attributes.iter().any(|field| {
        field.key.as_str() == "connection_id"
            && field.value == i64::from(fixture.connection.id).into()
    }));
    assert_eq!(operations.db.into_transaction_log().len(), 2);
}

#[tokio::test]
async fn failed_customer_validation_retains_its_action_parent() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_errors([sea_orm::DbErr::Custom(
            "fixture customer read failed".into(),
        )])
        .into_connection();
    let operations = StravaOperations::new(db);
    let trace = capture(async {
        assert!(operations.connection(&fixture.event).await.is_err());
    })
    .await;
    stage(&trace, VALIDATE);
    assert_eq!(operations.db.into_transaction_log().len(), 2);
}
