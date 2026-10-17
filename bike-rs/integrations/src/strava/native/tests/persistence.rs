use super::*;
use bike_core::entities::strava_events;
use sea_orm::{ActiveEnum, Iterable, MockExecResult, Value};
use std::collections::BTreeMap;

#[test]
fn activity_statuses_have_matching_wire_and_database_values() {
    for status in ActivityStatus::iter() {
        let wire = serde_json::to_value(status).unwrap();
        assert_eq!(wire, status.to_value());
        assert_eq!(serde_json::from_value::<ActivityStatus>(wire).unwrap(), status);
    }
    assert!(serde_json::from_str::<ActivityStatus>("\"not_a_status\"").is_err());
}

#[tokio::test]
async fn child_counts_filter_the_status_column_using_a_bound_enum() {
    let fixture = Fixture::new();
    let counts: BTreeMap<String, Value> = [("applied".into(), 2.into()), ("other".into(), 3.into())].into();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![counts]])
        .into_connection();
    assert_eq!(
        strava_events::Model::collection_counts(&db, &fixture.event)
            .await
            .unwrap(),
        (2, 3)
    );
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("status=$3"));
    assert!(log.contains("applied"));
    assert!(!log.contains("mutation->>"));
}

#[tokio::test]
async fn mutation_writes_status_and_retry_dependencies_together_without_json_status() {
    let fixture = Fixture::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
        .into_connection();
    strava_events::Model::record_mutation(&db, &fixture.change(ActivityStatus::Deleted))
        .await
        .unwrap();
    let transactions = db.into_transaction_log();
    assert_eq!(transactions.len(), 1);
    let statement = &transactions[0].statements()[0];
    let values = &statement.values.as_ref().unwrap().0;
    assert!(values.contains(&ActivityStatus::Deleted.into()));
    let mutation = values
        .iter()
        .find_map(|value| match value {
            Value::Json(Some(json)) => Some(json),
            _ => None,
        })
        .unwrap();
    assert!(mutation.get("segment_ids").is_some());
    assert!(mutation.get("status").is_none());
}

#[tokio::test]
async fn stored_mutation_uses_the_status_column_and_rejects_an_identity_mismatch() {
    let fixture = Fixture::new();
    let mut row = fixture.row(ActivityStatus::Deleted);
    row.mutation = Some(serde_json::to_value(fixture.change(ActivityStatus::Captured)).unwrap());
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row.clone()]])
        .into_connection();
    assert_eq!(
        strava_events::Model::stored_mutation(&db, &fixture.event)
            .await
            .unwrap()
            .unwrap()
            .status,
        ActivityStatus::Deleted
    );
    row.customer_id = uuid::Uuid::from_u128(99);
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row]])
        .into_connection();
    assert!(strava_events::Model::stored_mutation(&db, &fixture.event)
        .await
        .is_err());
}
