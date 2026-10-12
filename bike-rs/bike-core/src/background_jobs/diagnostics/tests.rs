use super::*;
use crate::background_jobs::{
    entities::work_units,
    execution::{measured, ExecutionContext, WorkCounts, WorkScope},
    history_tests::{database, enqueue},
    pipeline::PipelineContext,
};
use sea_orm::{ActiveModelTrait, PaginatorTrait, Set};
use std::sync::Arc;

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
    task.mark_completed(&db).await.unwrap();
    let graph = PipelineGraph::load(&db, &root.run_id, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(graph.tasks[0].work.len(), 2);
    assert_eq!(
        graph.tasks[0].work[0].counts.as_ref().unwrap()["inputs_read"],
        3
    );
    assert!(graph.tasks[0].anomalies.is_empty());
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
