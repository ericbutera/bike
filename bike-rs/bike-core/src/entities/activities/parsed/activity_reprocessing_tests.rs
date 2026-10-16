use super::*;
use crate::test_support as fixtures;
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn conditional_source_write_includes_the_version_it_read() {
    let activity = fixtures::reprocess_activity();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![activity.clone()]])
        .into_connection();
    let parsed = fixtures::parsed_reprocess_activity();
    let profile = TrainingProfile::default();
    let artifact = crate::activity_import_pipeline::load_best_activity_parsing_artifact(
        &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![fixtures::reprocess_artifact()]])
            .into_connection(),
        &fixtures::reprocess_import(),
    )
    .await
    .unwrap();
    activity
        .clone()
        .update_parsed(
            &db,
            &artifact,
            &parsed,
            build_save_data(&parsed, "outdoor.tcx", &profile).unwrap(),
        )
        .await
        .unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("WHERE"));
    assert!(log.contains("updated_at\\\" ="));
    let expected = format!("{:?}", sea_orm::Value::from(activity.updated_at));
    assert!(log.contains(&expected), "expected {expected} in {log}");
}
