use super::ProcessorSummary;
use crate::background_jobs::{
    entities::processor_registry,
    history_tests::{database, enqueue},
};

#[tokio::test]
async fn registered_processors_without_history_have_counts_and_absent_percentiles() {
    let db = database(true).await;
    processor_registry::Model::register(
        &db,
        &[
            "rebuild_fitness_freshness".into(),
            "receive_strava_delivery".into(),
        ],
    )
    .await
    .unwrap();
    let task = enqueue(&db).await;
    let before = ProcessorSummary::list(&db, 24, "completed").await.unwrap();
    let fitness = before
        .iter()
        .find(|row| row.task_type == task.task_type)
        .unwrap();
    assert_eq!(
        (fitness.queued, fitness.running, fitness.window_hours),
        (1, 0, 24)
    );
    assert_eq!(fitness.outcome, "completed");
    assert_eq!(fitness.attempt.samples, 0);
    assert_eq!(fitness.attempt.p50_seconds, None);
    assert_eq!(fitness.eligible_wait.p90_seconds, None);
    assert_eq!(fitness.logical_completion.p90_seconds, None);
    let claimed = task.claim(&db).await.unwrap().unwrap();
    let during = ProcessorSummary::list(&db, 1, "retrying").await.unwrap();
    let fitness = during
        .iter()
        .find(|row| row.task_type == task.task_type)
        .unwrap();
    assert_eq!((fitness.queued, fitness.running), (0, 1));
    claimed
        .mark_failed(&db, "provider unavailable".into())
        .await
        .unwrap();
    let after = ProcessorSummary::list(&db, 168, "retrying").await.unwrap();
    let fitness = after
        .iter()
        .find(|row| row.task_type == task.task_type)
        .unwrap();
    assert_eq!(fitness.retrying, 1);
    let unused = after
        .iter()
        .find(|row| row.task_type == "receive_strava_delivery")
        .unwrap();
    assert_eq!((unused.queued, unused.running, unused.failed), (0, 0, 0));
}
