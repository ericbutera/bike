use super::background_tasks;
use crate::test_support as fixtures;
use sea_orm::{DbErr, MockExecResult};

fn executing() -> background_tasks::Model {
    background_tasks::Model {
        status: "processing".into(),
        attempts: 1,
        ..fixtures::background_task()
    }
}

#[tokio::test]
async fn claim_compares_the_snapshot_and_commits_the_attempt_with_the_task() {
    let db = fixtures::postgres_mock()
        .append_exec_results([fixtures::written()])
        .append_query_results([vec![fixtures::task_attempt()]])
        .append_query_results([vec![executing()]])
        .into_connection();
    assert_eq!(fixtures::background_task().claim(&db).await.unwrap(), Some(executing()));
    let log = db.into_transaction_log();
    let statements = log[0].statements();
    assert_eq!(statements.first().unwrap().sql, "BEGIN");
    let predicate = statements[1].sql.split(" WHERE ").nth(1).unwrap();
    for field in [
        "\"id\" =",
        "\"status\" =",
        "\"attempts\" =",
        "\"updated_at\" =",
        "\"max_attempts\"",
        "\"scheduled_for\" <=",
    ] {
        assert!(predicate.contains(field), "{:?}", statements[1]);
    }
    assert!(statements[2]
        .sql
        .starts_with("INSERT INTO \"background_task_attempts\""));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn a_lost_claim_does_not_create_an_attempt_or_execute_the_task() {
    let db = fixtures::postgres_mock()
        .append_exec_results([MockExecResult::default()])
        .into_connection();
    assert!(fixtures::background_task().claim(&db).await.unwrap().is_none());
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().len(), 3);
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn a_failed_attempt_write_rolls_back_the_claim() {
    let failure = DbErr::Custom("fixture attempt unavailable".into());
    let db = fixtures::postgres_mock()
        .append_exec_results([fixtures::written()])
        .append_query_errors([failure.clone()])
        .into_connection();
    assert_eq!(fixtures::background_task().claim(&db).await.unwrap_err(), failure);
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn a_canceled_or_replaced_execution_cannot_report_completion() {
    let canceled = background_tasks::Model {
        status: "canceled".into(),
        ..executing()
    };
    let db = fixtures::postgres_mock()
        .append_exec_results([MockExecResult::default()])
        .append_query_results([vec![canceled]])
        .into_connection();
    assert!(executing().finish_execution(&db, Ok(())).await.unwrap().is_none());
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().len(), 4);
    let predicate = log[0].statements()[1].sql.split(" WHERE ").nth(1).unwrap();
    assert!(predicate.contains("\"id\" ="));
    assert!(predicate.contains("\"status\" =") && predicate.contains("\"attempts\" ="));
    assert!(!log[0]
        .statements()
        .iter()
        .any(|statement| statement.sql.contains("background_task_attempts")));
}

#[tokio::test]
async fn failed_executions_retry_until_the_attempt_budget_is_exhausted() {
    for (attempts, status, outcome) in [(1, "pending", "retrying"), (3, "failed", "failed")] {
        let task = background_tasks::Model {
            attempts,
            ..executing()
        };
        let db = fixtures::postgres_mock()
            .append_exec_results([fixtures::written(), fixtures::written()])
            .append_query_results([vec![background_tasks::Model {
                status: status.into(),
                ..task.clone()
            }]])
            .into_connection();
        assert!(task
            .finish_execution(&db, Err("fixture error".into()))
            .await
            .unwrap()
            .is_some());
        let log = db.into_transaction_log();
        assert!(log[0].statements()[1]
            .values
            .as_ref()
            .unwrap()
            .0
            .contains(&status.into()));
        assert!(log[0].statements()[2]
            .values
            .as_ref()
            .unwrap()
            .0
            .contains(&outcome.into()));
        assert_eq!(log[0].statements().last().unwrap().sql, "COMMIT");
    }
}

#[tokio::test]
async fn heartbeats_fence_the_attempt_and_do_not_advance_progress() {
    let db = fixtures::postgres_mock()
        .append_exec_results([fixtures::written(), fixtures::written()])
        .into_connection();
    assert!(executing().heartbeat(&db).await.unwrap());
    let log = db.into_transaction_log();
    let attempt = &log[0].statements()[2];
    assert!(attempt.sql.contains("\"heartbeat_at\" ="));
    assert!(attempt.sql.contains("\"attempt\" ="));
    assert!(!attempt.sql.contains("progress_at"));
}

#[tokio::test]
async fn stale_recovery_cannot_overwrite_a_new_heartbeat() {
    let db = fixtures::postgres_mock()
        .append_exec_results([MockExecResult::default()])
        .into_connection();
    assert!(!executing()
        .recover(&db, serde_json::json!({"source":"fixture"}))
        .await
        .unwrap());
    let log = db.into_transaction_log();
    let predicate = log[0].statements()[1].sql.split(" WHERE ").nth(1).unwrap();
    assert!(predicate.contains("\"updated_at\" ="));
    assert!(predicate.contains("\"id\" ="));
    assert!(predicate.contains("\"status\" =") && predicate.contains("\"attempts\" ="));
    assert_eq!(log[0].statements().len(), 3);
    assert_eq!(log[0].statements().last().unwrap().sql, "COMMIT");
}
