use super::*;
use crate::background_jobs::entities::task_attempts;
use crate::test_support as fixtures;

struct Storage(DatabaseConnection);
impl BackgroundTasksStorage for Storage {
    fn db(&self) -> &DatabaseConnection {
        &self.0
    }
}

struct VerifiedAdmin;
impl AdminVerified for VerifiedAdmin {}

#[tokio::test]
async fn legacy_task_detail_preserves_attempt_diagnostics_without_a_graph() {
    let finished_at = fixtures::timestamp() + chrono::Duration::seconds(1);
    let task = background_tasks::Model {
        status: "completed".into(),
        attempts: 1,
        completed_at: Some(finished_at),
        ..fixtures::background_task()
    };
    let attempt = task_attempts::Model {
        finished_at: Some(finished_at),
        outcome: "completed".into(),
        trace_id: Some("fixture-trace".into()),
        ..fixtures::task_attempt()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![task.clone()]])
        .append_query_results([vec![attempt.clone()]])
        .into_connection();
    let Json(detail) = get_task(VerifiedAdmin, State(Arc::new(Storage(db.clone()))), Path(task.id))
        .await
        .unwrap();
    assert_eq!(detail.id, task.id);
    assert_eq!(detail.status, "completed");
    assert_eq!(detail.attempt_history.len(), 1);
    let retained = &detail.attempt_history[0];
    assert_eq!(retained.outcome, attempt.outcome);
    assert_eq!(retained.finished_at, Some(finished_at.to_rfc3339()));
    assert_eq!(retained.trace_id, attempt.trace_id);
    let payload = serde_json::to_value(detail).unwrap();
    assert!(payload.get("graph").is_none());
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 2);
    assert_eq!(log[1].statements()[0].values.as_ref().unwrap().0, vec![task.id.into()]);
}
