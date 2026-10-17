use super::*;
use crate::{strava::native::strava_native_unit::fixture::Fixture, test_support as fixtures};
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn committed_outcome_is_rechecked_under_the_lock_before_classifying_a_retry() {
    let mut fixture = Fixture::new();
    fixture.event.operation = crate::strava::events::StravaOperation::Update;
    let mut original = ActivityChange::applied(
        fixture.event.clone(),
        fixture.activity().id,
        18,
        fixture.activity().started_at.date_naive(),
    );
    original.segment_ids = vec![1, 2];
    let mut committed = fixture.row(ActivityStatus::Applied);
    committed.mutation = Some(serde_json::to_value(&original).unwrap());
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![committed]])
        .into_connection();
    // Another attempt commits after this first lookup misses, before we get the lock.
    assert!(strava_events::Model::stored_mutation(&db, &fixture.event)
        .await
        .unwrap()
        .is_none());
    let operations = StravaOperations::new(db);
    let result = operations
        .persist_summary(
            fixture.normalized_activity(),
            fixtures::parsed_reprocess_activity(),
            serde_json::from_slice(crate::strava::fixtures::REAL_RIDE).unwrap(),
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    let logs = operations.db.into_transaction_log();
    let statements = logs.last().unwrap().statements();
    assert!(statements[1].sql.contains("FOR UPDATE"));
    assert!(statements[2].sql.contains("strava_events"));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
    assert_eq!(statements.len(), 4);
}

#[test]
fn captured_segment_scope_survives_a_lost_rematch_completion() {
    let fixture = Fixture::new();
    let captured = ActivityChange {
        segment_ids: vec![1, 2],
        ..fixture.change(ActivityStatus::Applied)
    };
    let mut first_attempt = captured.clone();
    // Matching reports old A/B and new B/C on the first attempt.
    first_attempt.include_segments([1, 2, 2, 3]);
    assert_eq!(first_attempt.segment_ids, vec![1, 2, 3]);
    // Temporal retries the original Activity input. The current rows now contain
    // only B/C, so its result must also retain the captured removed membership A.
    let mut retried = captured;
    retried.include_segments([2, 3]);
    assert_eq!(retried.segment_ids, first_attempt.segment_ids);
    retried.include_segments([]);
    assert_eq!(retried.segment_ids, vec![1, 2, 3]);
}
