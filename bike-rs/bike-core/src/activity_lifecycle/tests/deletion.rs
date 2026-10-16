use super::*;
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, MockExecResult, Statement, Value, Values};
use std::collections::BTreeMap;

fn deletion_mock() -> MockDatabase {
    let segments: Vec<BTreeMap<String, Value>> = [3, 2, 3]
        .into_iter()
        .map(|id| [("segment_id".into(), id.into())].into())
        .collect();
    MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([segments])
        .append_exec_results((0..4).map(|_| executed()))
}

fn executed() -> MockExecResult {
    MockExecResult {
        rows_affected: 1,
        last_insert_id: 0,
    }
}

fn import_count(count: i64) -> Vec<BTreeMap<String, Value>> {
    vec![[("num_items".into(), count.into())].into()]
}

fn activity() -> activities::Model {
    make_activity(42, Utc::now(), Some("gpx"), None, vec![])
}

fn statements(db: DatabaseConnection) -> Vec<Statement> {
    db.into_transaction_log()
        .into_iter()
        .flat_map(|transaction| transaction.statements().to_vec())
        .collect()
}

fn assert_owned_delete(statement: &Statement, table: &str, id: i32) {
    assert_eq!(
        statement.sql,
        format!("DELETE FROM \"{table}\" WHERE \"{table}\".\"id\" = $1 AND \"{table}\".\"user_id\" = $2")
    );
    assert_eq!(statement.values, Some(Values(vec![id.into(), 42.into()])));
}

#[tokio::test]
async fn deletion_removes_derived_rows_and_returns_affected_segments_without_an_import() {
    let db = deletion_mock().into_connection();
    let mut activity = activity();
    activity.activity_import_id = None;

    let deleted = delete_activity_records(&db, 42, activity).await.unwrap();

    assert_eq!(deleted.segment_ids, [2, 3]);
    assert_eq!(deleted.source, None);
    let log = statements(db);
    assert_eq!(log.len(), 5);
    for (statement, table) in log[2..4]
        .iter()
        .zip(["activity_analytics", "activity_training_analyses"])
    {
        assert_eq!(
            statement.sql,
            format!("DELETE FROM \"{table}\" WHERE \"{table}\".\"activity_id\" = $1")
        );
        assert_eq!(statement.values, Some(Values(vec![42.into()])));
    }
    assert_owned_delete(&log[4], "activities", 42);
}

#[tokio::test]
async fn deletion_retains_shared_imports_and_counts_only_the_owners_references() {
    let db = deletion_mock()
        .append_query_results([import_count(1)])
        .into_connection();

    let deleted = delete_activity_records(&db, 42, activity()).await.unwrap();

    assert_eq!(deleted.source, None);
    let log = statements(db);
    assert_eq!(log.len(), 6);
    assert!(log[5].sql.contains("\"activities\".\"user_id\" = $1"));
    assert!(log[5]
        .sql
        .contains("\"activities\".\"activity_import_id\" = $2"));
    assert_eq!(log[5].values, Some(Values(vec![42.into(), 1042.into()])));
}

#[tokio::test]
async fn deletion_removes_the_last_owned_import_and_returns_its_source() {
    let import = activity_imports::Model {
        id: 1042,
        user_id: 42,
        activity_id: Some(42),
        ..crate::test_support::activity_import()
    };
    let db = deletion_mock()
        .append_query_results([import_count(0)])
        .append_query_results([vec![import.clone()]])
        .append_exec_results([executed()])
        .into_connection();

    let deleted = delete_activity_records(&db, 42, activity()).await.unwrap();

    assert_eq!(deleted.source, Some(import.storage_path));
    let log = statements(db);
    assert_eq!(log.len(), 8);
    assert!(log[6].sql.contains("\"activity_imports\".\"id\" = $1"));
    assert!(log[6].sql.contains("\"activity_imports\".\"user_id\" = $2"));
    assert_eq!(
        log[6].values,
        Some(Values(vec![import.id.into(), 42.into(), 1_u64.into()]))
    );
    assert_owned_delete(&log[7], "activity_imports", import.id);
}

#[tokio::test]
async fn deletion_tolerates_an_import_missing_from_the_owners_scope() {
    let db = deletion_mock()
        .append_query_results([import_count(0)])
        .append_query_results([Vec::<activity_imports::Model>::new()])
        .into_connection();

    let deleted = delete_activity_records(&db, 42, activity()).await.unwrap();

    assert_eq!(deleted.source, None);
    assert_eq!(statements(db).len(), 7);
}

#[tokio::test]
async fn deletion_rolls_back_when_counting_remaining_import_references_fails() {
    let db = deletion_mock()
        .append_query_errors([DbErr::Custom("fixture count failure".into())])
        .into_connection();

    let error = delete_activity_with_derived_state(&db, "/unused", 42, activity())
        .await
        .expect_err("reference count must succeed before committing deletion");

    assert_eq!(error.message, "Database request failed");
    let log = statements(db);
    assert_eq!(log.first().unwrap().sql, "BEGIN");
    assert_eq!(log.last().unwrap().sql, "ROLLBACK");
    assert!(!log.iter().any(|statement| statement.sql == "COMMIT"));
}
