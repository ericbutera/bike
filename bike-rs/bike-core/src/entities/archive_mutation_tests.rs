use super::activity_archive_import_jobs::{Entity, Model};
use crate::{archive_import::ActivityArchiveImportResponse, test_support as fixtures};

fn progress() -> ActivityArchiveImportResponse {
    ActivityArchiveImportResponse {
        source: "https://example.test/retained.zip".into(),
        total_entries: 5,
        supported_entry_count: 4,
        imported_count: 2,
        duplicate_count: 1,
        skipped_unsupported_count: 1,
        failed_count: 1,
        error_samples: vec!["fixture corrupt entry".into()],
    }
}

#[tokio::test]
async fn archive_progress_preserves_running_status_and_requires_the_owned_job() {
    let db = fixtures::postgres_mock()
        .append_exec_results([fixtures::written()])
        .into_connection();
    Entity::store_progress(&db, 7, 20, &progress()).await.unwrap();
    let log = db.into_transaction_log();
    let write = &log[0].statements()[0];
    assert!(!write.sql.contains("\"status\" ="));
    assert!(!write.sql.contains("finished_at"));
    assert!(write.sql.contains("\"user_id\" ="));
    assert!(write.values.as_ref().unwrap().0.contains(&2.into()));
    let db = fixtures::postgres_mock()
        .append_exec_results([sea_orm::MockExecResult::default()])
        .into_connection();
    assert!(matches!(
        Entity::store_progress(&db, 7, 20, &progress()).await,
        Err(sea_orm::DbErr::RecordNotFound(_))
    ));
}

#[tokio::test]
async fn restarting_an_archive_resets_progress_and_error_state_without_replacing_source_identity() {
    let job = fixtures::archive_import_job();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![job.clone()]])
        .into_connection();
    Model::mark_running(&db, &job).await.unwrap();
    let log = db.into_transaction_log();
    let write = &log[0].statements()[0];
    let update = write.sql.split(" RETURNING ").next().unwrap();
    for field in [
        "\"failed_count\" =",
        "\"imported_count\" =",
        "\"error_samples_json\" =",
        "\"finished_at\" =",
    ] {
        assert!(update.contains(field), "{write:?}");
    }
    assert!(!update.contains("\"archive_url\" ="));
    assert!(write.values.as_ref().unwrap().0.contains(&"running".into()));
}

#[tokio::test]
async fn archive_completion_preserves_partial_failure_counts_and_samples() {
    let job = fixtures::archive_import_job();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![job.clone()]])
        .into_connection();
    Model::mark_finished(&db, &job, &progress()).await.unwrap();
    let log = db.into_transaction_log();
    let write = &log[0].statements()[0];
    assert!(write.sql.contains("\"finished_at\" ="));
    assert!(write.values.as_ref().unwrap().0.contains(&"partial".into()));
    assert!(write
        .values
        .as_ref()
        .unwrap()
        .0
        .contains(&"[\"fixture corrupt entry\"]".into()));
}

#[tokio::test]
async fn archive_failure_keeps_prior_progress_for_diagnosis() {
    let job = fixtures::archive_import_job();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![job.clone()]])
        .into_connection();
    Model::mark_failed(&db, &job, None, "fixture download failed".into())
        .await
        .unwrap();
    let log = db.into_transaction_log();
    let write = &log[0].statements()[0];
    assert!(!write
        .sql
        .split(" RETURNING ")
        .next()
        .unwrap()
        .contains("\"imported_count\" ="));
    assert!(write
        .values
        .as_ref()
        .unwrap()
        .0
        .contains(&"fixture download failed".into()));
}
