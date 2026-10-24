use super::*;
use crate::test_support as fixtures;
use crate::test_support::strava_fixture::Fixture;
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn committed_outcome_is_rechecked_under_the_lock_before_classifying_a_retry() {
    let mut fixture = Fixture::new();
    fixture.event.operation = crate::entities::strava_events::StravaOperation::Update;
    fixture.event.source_version = Some(Default::default());
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
    let objects = fixtures::object_storage(crate::object_store::ObjectOwner::Strava).0;
    let input = fixture.normalized_activity();
    let result = Model::apply_strava_source(
        &db,
        &objects,
        RetainedActivity {
            event: input.event,
            source: input.source,
        },
        &fixtures::parsed_reprocess_activity(),
        &Default::default(),
        &crate::activity_import_pipeline::ActivityUploadPayload {
            original_filename: "fixture.json".into(),
            format: "json".into(),
            mime_type: None,
            source_correlation_id: None,
            bytes: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        serde_json::to_value(original).unwrap()
    );
    let logs = db.into_transaction_log();
    let statements = logs.last().unwrap().statements();
    assert!(statements[1].sql.contains("FOR UPDATE"));
    assert!(statements[2].sql.contains("strava_events"));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
    assert_eq!(statements.len(), 4);
}

#[tokio::test]
async fn a_delayed_fetched_snapshot_never_mutates_or_records_an_immutable_outcome() {
    let mut fixture = Fixture::new();
    fixture.event.source_version = Some(crate::entities::strava_events::SourceVersion { updated_at: None });
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![fixture.connection.clone()]])
        .append_query_results([vec![fixture.row(ActivityStatus::Pending)]])
        .append_query_results([vec![fixture.activity_state()]])
        .into_connection();
    let objects = fixtures::object_storage(crate::object_store::ObjectOwner::Strava).0;
    let input = fixture.normalized_activity();
    let result = Model::apply_strava_source(
        &db,
        &objects,
        RetainedActivity {
            event: input.event,
            source: input.source,
        },
        &fixtures::parsed_reprocess_activity(),
        &Default::default(),
        &crate::activity_import_pipeline::ActivityUploadPayload {
            original_filename: "fixture.json".into(),
            format: "json".into(),
            mime_type: None,
            source_correlation_id: None,
            bytes: vec![],
        },
    )
    .await
    .unwrap();
    assert_eq!(result.status, ActivityStatus::SourceChanged);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(!log.contains("INSERT INTO"));
    assert!(!log.contains("UPDATE \""));
    assert!(log.contains("COMMIT"));
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
