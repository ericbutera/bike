use super::activity_imports;
use crate::test_support as fixtures;
use sea_orm::{DatabaseConnection, DbErr, Statement, Value};

fn last_write(db: DatabaseConnection) -> Statement {
    db.into_transaction_log().last().unwrap().statements()[0].clone()
}

#[tokio::test]
async fn retrying_processing_clears_the_error_without_resetting_identity_or_attempts() {
    let import = activity_imports::Model {
        activity_id: Some(42),
        processing_attempts: 2,
        processing_error: Some("previous failure".into()),
        ..fixtures::activity_import()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![import.clone()]])
        .into_connection();
    import
        .mark_processing(&db, "activity_parsed", None, fixtures::timestamp())
        .await
        .unwrap();
    let write = last_write(db);
    let update = write.sql.split(" RETURNING ").next().unwrap();
    for untouched in [
        "\"activity_id\" =",
        "\"processing_attempts\" =",
        "\"source\" =",
        "\"storage_path\" =",
    ] {
        assert!(!update.contains(untouched), "{write:?}");
    }
    for value in [Value::from("processing"), "activity_parsed".into(), Value::String(None)] {
        assert!(write.values.as_ref().unwrap().0.contains(&value), "{write:?}");
    }
}

#[tokio::test]
async fn failed_import_increments_attempts_without_overflowing_or_clearing_its_source() {
    for attempts in [2, i32::MAX] {
        let import = activity_imports::Model {
            processing_attempts: attempts,
            ..fixtures::activity_import()
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![import.clone()]])
            .into_connection();
        import
            .mark_failed(&db, "activity_parsed", "fixture parse failure")
            .await
            .unwrap();
        let write = last_write(db);
        let values = &write.values.as_ref().unwrap().0;
        for value in [
            "failed".into(),
            "fixture parse failure".into(),
            attempts.saturating_add(1).into(),
        ] {
            assert!(values.contains(&value), "{write:?}");
        }
        assert!(!write
            .sql
            .split(" RETURNING ")
            .next()
            .unwrap()
            .contains("\"storage_path\" ="));
    }
}

#[tokio::test]
async fn native_completion_keeps_legacy_processing_history_and_import_identity() {
    let import = fixtures::activity_import();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![import.clone()]])
        .into_connection();
    import.mark_native_processed(&db).await.unwrap();
    let write = last_write(db);
    let update = write.sql.split(" RETURNING ").next().unwrap();
    for untouched in [
        "\"activity_id\" =",
        "\"last_processing_event_at\" =",
        "\"processing_attempts\" =",
    ] {
        assert!(!update.contains(untouched), "{write:?}");
    }
    assert!(update.contains("\"processed_at\" ="));
    assert!(write.values.as_ref().unwrap().0.contains(&"processed".into()));
}

#[tokio::test]
async fn duplicate_import_links_the_canonical_activity_and_finishes_processing() {
    let import = fixtures::activity_import();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![import.clone()]])
        .into_connection();
    import.mark_duplicate(&db, 99).await.unwrap();
    let write = last_write(db);
    for value in ["duplicate".into(), "complete".into(), 99.into()] {
        assert!(write.values.as_ref().unwrap().0.contains(&value), "{write:?}");
    }
    assert!(write.sql.contains("\"processed_at\" ="));
}

#[tokio::test]
async fn native_reprocess_completion_is_scoped_to_the_owned_import_and_checks_affected_rows() {
    let execution = fixtures::reprocess_execution();
    let db = fixtures::postgres_mock()
        .append_exec_results([fixtures::written()])
        .into_connection();
    activity_imports::Model::complete_native_reprocess(&db, &execution)
        .await
        .unwrap();
    let write = last_write(db);
    assert!(write.sql.contains("\"id\" =") && write.sql.contains("\"user_id\" ="));
    let values = &write.values.as_ref().unwrap().0;
    assert!(values.contains(&execution.command.source.user_id.into()));
    assert!(values.contains(&execution.command.source.activity_import_id.into()));
    let db = fixtures::postgres_mock()
        .append_exec_results([sea_orm::MockExecResult::default()])
        .into_connection();
    let error = activity_imports::Model::complete_native_reprocess(&db, &execution)
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn import_mutation_propagates_storage_failure() {
    let expected = DbErr::Custom("fixture database unavailable".into());
    let db = fixtures::postgres_mock()
        .append_query_errors([expected.clone()])
        .into_connection();
    assert_eq!(
        fixtures::activity_import().mark_processed(&db).await.unwrap_err(),
        expected
    );
}

#[tokio::test]
async fn copied_artifacts_allocate_a_new_key_and_keep_the_retained_source_metadata() {
    for resize in [false, true] {
        let original = fixtures::import_artifact();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![original.clone()]])
            .into_connection();
        if resize {
            original.clone().copy_with_size(&db, 999).await.unwrap();
        } else {
            original.clone().copy_to_import(&db, 77).await.unwrap();
        }
        let write = last_write(db);
        let insert = write.sql.split(" VALUES ").next().unwrap();
        assert!(!insert.contains("\"id\""), "{write:?}");
        for value in [
            original.storage_path.into(),
            original.checksum_sha256.into(),
            if resize { 999_i64.into() } else { 77.into() },
        ] {
            assert!(write.values.as_ref().unwrap().0.contains(&value), "{write:?}");
        }
    }
}
