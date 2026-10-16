use crate::tasks::{register_default_processors, ProcessActivityImport, ReprocessActivityImport};
use bike_core::{
    background_jobs::worker::{TaskProcessor, TaskWorker},
    entities::{activities, activity_imports},
    test_support as fixtures,
};
use serde_json::json;

#[tokio::test]
async fn the_legacy_registry_keeps_all_remaining_tasks_until_their_cutover() {
    let db = fixtures::postgres_mock().into_connection();
    let worker = register_default_processors(TaskWorker::new(db.clone()), db.clone())
        .await
        .unwrap();
    let mut actual = worker.registered_task_types();
    actual.sort();
    let mut expected = vec![
        "backfill_user_xc_training",
        "rebuild_fitness_freshness",
        "rebuild_segment_analytics",
        "regenerate_segment_efforts",
        "process_activity_import",
        "reprocess_activity_import",
        "reprocess_user_activity_imports",
        "reprocess_archive_fit_activity_imports",
        "regenerate_user_segments",
        "regenerate_activity_segments",
        "activity_archive_import",
        "prepare_heatmap",
    ];
    expected.sort();
    assert_eq!(actual, expected);
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn missing_upload_source_fails_without_starting_an_attempt_or_reading_files() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<activity_imports::Model>::new()])
        .into_connection();
    let processor = ProcessActivityImport::new(db.clone());
    let result = processor
        .process(5, json!({"data":{"user_id":7,"import_id":18}}))
        .await;
    assert!(result.unwrap_err().to_string().contains("not found"));
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn missing_reprocess_target_fails_without_creating_an_activity() {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<activities::Model>::new()])
        .into_connection();
    let processor = ReprocessActivityImport::new(db.clone());
    let result = processor.process(5, json!({"activity_id":42})).await;
    assert!(result.unwrap_err().to_string().contains("not found"));
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn malformed_import_commands_fail_before_reading_the_database() {
    let db = fixtures::postgres_mock().into_connection();
    assert!(ProcessActivityImport::new(db.clone())
        .process(5, json!({"data":{}}))
        .await
        .is_err());
    assert!(ReprocessActivityImport::new(db.clone())
        .process(5, json!({}))
        .await
        .is_err());
    assert!(db.into_transaction_log().is_empty());
}
