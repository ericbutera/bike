use super::*;
use crate::background_jobs::{
    entities::{processor_registry, task_anomalies, task_attempts, work_units},
    execution::{measured, ExecutionContext, WorkCounts, WorkScope},
    history_tests::{database, enqueue},
    pipeline::PipelineContext,
};
use sea_orm::{ActiveModelTrait, PaginatorTrait, Set};
use std::sync::Arc;

#[tokio::test]
async fn cancellation_resolves_operational_alerts_and_preserves_the_evidence() {
    let db = database(true).await;
    let task = enqueue(&db).await.claim(&db).await.unwrap().unwrap();
    task_attempts::Entity::update_many()
        .col_expr(
            task_attempts::Column::ProgressAt,
            Expr::value(Utc::now() - chrono::Duration::hours(2)),
        )
        .filter(task_attempts::Column::TaskId.eq(task.id))
        .exec(&db)
        .await
        .unwrap();
    let metrics = WorkerMetrics::new("cancel_anomaly_test");
    anomalies::evaluate(&db, &metrics).await.unwrap();
    assert_eq!(
        task_anomalies::Model::active_counts(&db).await.unwrap()[0].reason,
        "progress_stale"
    );
    task.cancel(&db).await.unwrap();
    anomalies::evaluate(&db, &metrics).await.unwrap();
    assert!(task_anomalies::Model::active_counts(&db)
        .await
        .unwrap()
        .is_empty());
    let evidence = task_anomalies::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(evidence.resolved_at.is_some());
    assert!(evidence.observed > evidence.expected);
}

#[tokio::test]
async fn intentional_rescheduling_resolves_a_prior_queue_budget_alert() {
    let db = database(true).await;
    let task = enqueue(&db).await;
    background_tasks::Entity::update_many()
        .col_expr(
            background_tasks::Column::UpdatedAt,
            Expr::value(Utc::now() - chrono::Duration::hours(2)),
        )
        .filter(background_tasks::Column::Id.eq(task.id))
        .exec(&db)
        .await
        .unwrap();
    let metrics = WorkerMetrics::new("schedule_anomaly_test");
    anomalies::evaluate(&db, &metrics).await.unwrap();
    assert_eq!(
        task_anomalies::Model::active_counts(&db).await.unwrap()[0].reason,
        "queue_budget"
    );
    background_tasks::Entity::update_many()
        .col_expr(
            background_tasks::Column::ScheduledFor,
            Expr::value(Utc::now() + chrono::Duration::days(1)),
        )
        .filter(background_tasks::Column::Id.eq(task.id))
        .exec(&db)
        .await
        .unwrap();
    anomalies::evaluate(&db, &metrics).await.unwrap();
    assert!(task_anomalies::Model::active_counts(&db)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn diagnostic_sampling_is_fair_eligible_and_never_loads_source_payloads() {
    let db = database(true).await;
    let mut future_ids = Vec::new();
    for _ in 0..6 {
        future_ids.push(
            background_tasks::Model::enqueue(
                &db,
                "rebuild_fitness_freshness".into(),
                serde_json::json!({}),
                Some(Utc::now() + chrono::Duration::days(1)),
                3,
            )
            .await
            .unwrap()
            .id,
        );
    }
    let origin = PipelineContext::received("request", Some("compact-source".into()));
    for _ in 0..6 {
        PipelineContext::scope(
            Some(origin.clone()),
            background_tasks::Model::enqueue(
                &db,
                "rebuild_fitness_freshness".into(),
                serde_json::json!({"data":{"source":"private route".repeat(10_000)}}),
                None,
                3,
            ),
        )
        .await
        .unwrap();
    }
    let other = background_tasks::Model::enqueue(
        &db,
        "email_notification".into(),
        serde_json::json!({"private":"source"}),
        None,
        3,
    )
    .await
    .unwrap()
    .claim(&db)
    .await
    .unwrap()
    .unwrap();
    let candidates = background_tasks::Model::diagnostic_candidates(&db)
        .await
        .unwrap();
    assert_eq!(candidates.len(), 5);
    assert!(candidates.iter().any(|task| task.id == other.id));
    assert!(candidates.iter().all(|task| !future_ids.contains(&task.id)
        && task.payload == serde_json::json!({})
        && task.result.is_none()));
    let queued = candidates
        .iter()
        .find(|task| task.status == "pending")
        .unwrap();
    let origins = pipeline_tasks::Model::origins(&db, queued.id)
        .await
        .unwrap();
    assert_eq!(
        origins[0].1.as_ref().unwrap().request_id.as_deref(),
        Some("compact-source")
    );
}

#[tokio::test]
async fn readiness_requires_matching_revisions_and_all_children() {
    let db = database(true).await;
    let origin = PipelineContext::received("request", Some("ready-request".into()));
    let parent = PipelineContext::scope(Some(origin.clone()), enqueue(&db)).await;
    PipelineContext::scope(
        Some(origin.clone()),
        pipeline_outputs::Model::require(&db, "activity", 7, "source-2".into()),
    )
    .await
    .unwrap();
    pipeline_outputs::Model::publish(&db, "activity", 7, "source-1", Utc::now())
        .await
        .unwrap();
    let metrics = WorkerMetrics::new("readiness_test");
    parent
        .claim(&db)
        .await
        .unwrap()
        .unwrap()
        .mark_completed(&db)
        .await
        .unwrap();
    reconcile_available(&db, &metrics).await.unwrap();
    assert!(pipeline_runs::Entity::find_by_id(&origin.run_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .available_at
        .is_none());
    let child =
        PipelineContext::scope(Some(origin.clone().for_task(parent.id)), enqueue(&db)).await;
    let published = Utc::now();
    pipeline_outputs::Model::publish(&db, "activity", 7, "source-2", published)
        .await
        .unwrap();
    reconcile_available(&db, &metrics).await.unwrap();
    assert!(pipeline_runs::Entity::find_by_id(&origin.run_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .available_at
        .is_none());
    child
        .claim(&db)
        .await
        .unwrap()
        .unwrap()
        .mark_completed(&db)
        .await
        .unwrap();
    reconcile_available(&db, &metrics).await.unwrap();
    reconcile_available(&db, &metrics).await.unwrap();
    assert_eq!(
        pipeline_runs::Entity::find_by_id(&origin.run_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .available_at,
        Some(published)
    );
    let graph = PipelineGraph::load(&db, &origin.run_id, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(graph.tasks.len(), 2);
    assert_eq!(graph.tasks[1].parent_task_id, Some(parent.id));
    assert_eq!(graph.outputs[0].revision, "source-2");
}

#[tokio::test]
async fn shared_output_execution_preserves_both_run_origins() {
    let db = database(true).await;
    let left = PipelineContext::received("request", None);
    let right = PipelineContext::received("request", None);
    for context in [&left, &right] {
        PipelineContext::scope(
            Some(context.clone()),
            pipeline_outputs::Model::require(&db, "heatmap", 42, "9:6".into()),
        )
        .await
        .unwrap();
    }
    let shared = enqueue(&db).await;
    pipeline_outputs::Model::attach_waiting_tasks(&db, "heatmap", 42, "9:6", shared.id)
        .await
        .unwrap();
    let roots = pipeline_tasks::Model::origins(&db, shared.id)
        .await
        .unwrap();
    assert_eq!(roots.len(), 2);
    shared
        .claim(&db)
        .await
        .unwrap()
        .unwrap()
        .mark_completed(&db)
        .await
        .unwrap();
    pipeline_outputs::Model::publish(&db, "heatmap", 42, "9:6", Utc::now())
        .await
        .unwrap();
    reconcile_available(&db, &WorkerMetrics::new("shared_test"))
        .await
        .unwrap();
    for context in [left, right] {
        assert!(PipelineGraph::load(&db, &context.run_id, 0)
            .await
            .unwrap()
            .unwrap()
            .available_at
            .is_some());
    }
}

#[tokio::test]
async fn heartbeat_does_not_hide_missing_progress_and_anomalies_deduplicate() {
    let db = database(true).await;
    let task = enqueue(&db).await.claim(&db).await.unwrap().unwrap();
    task_attempts::Entity::update_many()
        .col_expr(
            task_attempts::Column::ProgressAt,
            Expr::value(Utc::now() - chrono::Duration::hours(2)),
        )
        .filter(task_attempts::Column::TaskId.eq(task.id))
        .exec(&db)
        .await
        .unwrap();
    task.heartbeat(&db).await.unwrap();
    let metrics = WorkerMetrics::new("anomaly_test");
    let types = vec![task.task_type.clone(), "email_notification".into()];
    processor_registry::Model::register(&db, &types)
        .await
        .unwrap();
    sample(&db, &metrics, &types).await.unwrap();
    sample(&db, &metrics, &types).await.unwrap();
    let active = task_anomalies::Entity::find().all(&db).await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].reason, "progress_stale");
    assert_eq!(
        processor_registry::Entity::find().count(&db).await.unwrap(),
        2
    );
    task_attempts::Model::progress(&db, task.id, task.attempts)
        .await
        .unwrap();
    sample(&db, &metrics, &types).await.unwrap();
    assert!(task_anomalies::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .resolved_at
        .is_some());
}

#[tokio::test]
async fn actual_work_counts_once_and_graph_contains_compact_evidence() {
    let db = database(true).await;
    let root = PipelineContext::received("request", None);
    let task = PipelineContext::scope(Some(root.clone()), enqueue(&db))
        .await
        .claim(&db)
        .await
        .unwrap()
        .unwrap();
    let metrics = Arc::new(WorkerMetrics::new("work_test"));
    let context = ExecutionContext {
        task_id: task.id,
        attempt: task.attempts,
        task_type: task.task_type.clone(),
        metrics: Some(metrics),
    };
    for _ in 0..2 {
        context
            .clone()
            .scope(measured(
                &db,
                WorkScope {
                    kind: "fitness",
                    work_key: "user:7".into(),
                    revision: "input:day".into(),
                    mode: "incremental",
                    reason: "source_change",
                },
                async {
                    Ok::<_, DbErr>((
                        (),
                        WorkCounts {
                            inputs_read: 3,
                            units_computed: 2,
                            rows_written: 2,
                            outputs_published: 1,
                            outcome: "published",
                        },
                    ))
                },
            ))
            .await
            .unwrap();
    }
    assert_eq!(work_units::Entity::find().count(&db).await.unwrap(), 2);
    let task = task.mark_completed(&db).await.unwrap();
    completed(&db, &WorkerMetrics::new("repeat_test"), &task)
        .await
        .unwrap();
    let graph = PipelineGraph::load(&db, &root.run_id, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(graph.tasks[0].work.len(), 2);
    assert_eq!(
        graph.tasks[0].work[0].counts.as_ref().unwrap()["inputs_read"],
        3
    );
    assert_eq!(graph.tasks[0].anomalies[0].reason, "repeated_revision");
}

#[tokio::test]
async fn activity_lookup_uses_direct_lineage_and_retention_protects_unready_work() {
    let db = database(true).await;
    let left = PipelineContext::received("request", None);
    let right = PipelineContext::received("request", None);
    for context in [&left, &right] {
        let task = PipelineContext::scope(Some(context.clone()), enqueue(&db)).await;
        if context.run_id == right.run_id {
            task.claim(&db)
                .await
                .unwrap()
                .unwrap()
                .mark_completed(&db)
                .await
                .unwrap();
        }
    }
    super::super::entities::pipeline_subjects::Model::attach(&db, &left.run_id, "activity", 7)
        .await
        .unwrap();
    super::super::entities::pipeline_subjects::Model::attach(&db, &right.run_id, "activity", 8)
        .await
        .unwrap();
    assert_eq!(
        PipelinePage::for_activity(&db, 7, None)
            .await
            .unwrap()
            .run_ids,
        vec![left.run_id.clone()]
    );
    let old = Utc::now() - chrono::Duration::days(31);
    let mut closed: pipeline_runs::ActiveModel = pipeline_runs::Entity::find_by_id(&right.run_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .into();
    closed.available_at = Set(Some(old));
    closed.update(&db).await.unwrap();
    retain(&db).await.unwrap();
    assert!(pipeline_runs::Entity::find_by_id(&left.run_id)
        .one(&db)
        .await
        .unwrap()
        .is_some());
    assert!(pipeline_runs::Entity::find_by_id(&right.run_id)
        .one(&db)
        .await
        .unwrap()
        .is_none());
}
