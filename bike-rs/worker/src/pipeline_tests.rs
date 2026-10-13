use crate::tasks::strava::{LegacyWorkflow, StravaReconcile, StravaRetention, StravaSync};
use crate::{pipeline_fixture_tests::database, tasks::*};
use bike_core::{
    background_jobs::{
        entities::{background_tasks, pipeline_outputs, work_units, worker_batches},
        execution::ExecutionContext,
        pipeline::PipelineContext,
        worker::{TaskProcessor, TaskWorker},
    },
    entities::{activities, activity_imports, segment_summaries, segments},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde_json::json;

#[tokio::test]
async fn pipeline_strava_sync_and_reconcile_share_one_native_workflow() {
    use bike_core::entities::{strava_connections, strava_workflows};
    let db = database().await;
    let connection = strava_connections::ActiveModel {
        user_id: Set(1),
        athlete_id: Set(35999641),
        access_token: Set("fixture-access".into()),
        refresh_token: Set("fixture-refresh".into()),
        scopes: Set("activity:read_all".into()),
        expires_at: Set(Utc::now() + chrono::Duration::hours(1)),
        last_sync_status: Set("never".into()),
        last_sync_imported_count: Set(0),
        last_sync_duplicate_count: Set(0),
        last_sync_failed_count: Set(0),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let sync = StravaSync::new(db.clone());
    sync.process(1, json!({"data":{"connection_id":connection.id}}))
        .await
        .unwrap();
    let reconcile = StravaReconcile::new(db.clone());
    assert_eq!(reconcile.schedule(), Some("0 0 */6 * * *"));
    reconcile.process(2, json!({})).await.unwrap();
    reconcile.process(3, json!({})).await.unwrap();
    let workflows = strava_workflows::Entity::find().all(&db).await.unwrap();
    assert_eq!(workflows.len(), 1);
    assert_eq!(workflows[0].action, "sync");
    assert_eq!(workflows[0].athlete_id, connection.athlete_id);
    assert_eq!(
        background_tasks::Entity::find().count(&db).await.unwrap(),
        1
    );
}

#[tokio::test]
async fn pipeline_strava_retention_startup_queues_hourly_maintenance() {
    use bike_core::background_jobs::worker::WorkerStartupHook;
    let db = database().await;
    let retention = StravaRetention::new(db.clone());
    assert_eq!(retention.task_type(), "strava_retention");
    assert_eq!(retention.schedule(), Some("0 0 * * * *"));
    let context = PipelineContext::received("worker_startup", None);
    PipelineContext::scope(
        Some(context.clone()),
        WorkerStartupHook::run(&retention, &db),
    )
    .await
    .unwrap();
    let job = background_tasks::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.task_type, "strava_retention");
    assert_eq!(job.status, "pending");
    assert_eq!(
        PipelineContext::from_payload(&job.payload)
            .unwrap()
            .unwrap()
            .run_id,
        context.run_id
    );
}

async fn enqueue_in_pipeline(
    db: &sea_orm::DatabaseConnection,
    origin: &PipelineContext,
    task_type: &str,
    payload: serde_json::Value,
) -> background_tasks::Model {
    PipelineContext::scope(
        Some(origin.clone()),
        background_tasks::Model::enqueue(db, task_type.into(), payload, None, 3),
    )
    .await
    .unwrap()
}

async fn seed_activity(db: &sea_orm::DatabaseConnection, id: i32) {
    activity_imports::ActiveModel {
        id: Set(id),
        user_id: Set(7),
        source: Set("archive_url_import".into()),
        format: Set("fit".into()),
        status: Set("processed".into()),
        import_version: Set(2),
        processing_stage: Set("completed".into()),
        processing_attempts: Set(1),
        original_filename: Set(format!("fixture-{id}.fit")),
        storage_path: Set(format!("fixture-{id}.fit")),
        size_bytes: Set(1),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    activities::ActiveModel {
        id: Set(id),
        user_id: Set(7),
        activity_import_id: Set(Some(id)),
        title: Set(format!("Fixture {id}")),
        sport: Set("ride".into()),
        source: Set("archive_url_import".into()),
        activity_type: Set("training".into()),
        started_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
}

async fn assert_batch_cursor(db: &sea_orm::DatabaseConnection, id: i32, cursor: i32, sealed: bool) {
    let batch = worker_batches::Entity::find_by_id(id)
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(batch.cursor, cursor);
    assert_eq!(batch.sealed, sealed);
}

#[tokio::test]
async fn pipeline_segment_cache_publishes_the_requested_revision_and_actual_work_counts() {
    let db = database().await;
    let revision = Utc::now();
    let segment = segments::ActiveModel {
        user_id: Set(7),
        title: Set("Fixture segment".into()),
        source: Set("manual".into()),
        mode: Set("cycling".into()),
        starred: Set(false),
        last_activity_change_at: Set(revision),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let origin = PipelineContext::received("admin", None);
    let task = enqueue_in_pipeline(
        &db,
        &origin,
        "rebuild_segment_analytics",
        json!({"data":{"segment_ids":[segment.id]}}),
    )
    .await
    .claim(&db)
    .await
    .unwrap()
    .unwrap();
    let processor = RebuildSegmentAnalytics::new(db.clone());
    PipelineContext::scope(
        Some(origin.for_task(task.id)),
        ExecutionContext {
            task_id: task.id,
            attempt: task.attempts,
            task_type: task.task_type.clone(),
            metrics: None,
        }
        .scope(processor.process(task.id, task.payload.clone())),
    )
    .await
    .unwrap();
    let summary = segment_summaries::Entity::find_by_id(segment.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.effort_count, 0);
    let output = pipeline_outputs::Entity::find()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(output.revision, revision.timestamp_micros().to_string());
    assert!(output.available_at.is_some());
    let work = work_units::Entity::find().one(&db).await.unwrap().unwrap();
    assert_eq!(work.outcome, "published");
    assert_eq!(
        work.counts.unwrap(),
        json!({"inputs_read":2,"units_computed":0,"rows_written":1,"outputs_published":1,"outcome":"published"})
    );
    assert_eq!(work.task_id, task.id);
    assert_eq!(work.attempt, 1);
}

#[tokio::test]
async fn pipeline_registry_and_bulk_processors_enqueue_pages_without_waiting_for_children() {
    for kind in ["reprocess", "archive_fit", "segments"] {
        let db = database().await;
        for id in 1..=18 {
            seed_activity(&db, id).await;
        }
        let processor: Box<dyn TaskProcessor> = match kind {
            "segments" => Box::new(RegenerateUserSegments::new(db.clone())),
            "archive_fit" => Box::new(ReprocessUserActivityImports::archive_fits_only(db.clone())),
            _ => Box::new(ReprocessUserActivityImports::new(db.clone())),
        };
        let origin = PipelineContext::received("admin", None);
        let parent = enqueue_in_pipeline(
            &db,
            &origin,
            processor.task_type(),
            json!({"data":{"user_id":7}}),
        )
        .await;
        PipelineContext::scope(
            Some(origin.clone().for_task(parent.id)),
            processor.process(parent.id, parent.payload),
        )
        .await
        .unwrap();
        assert_batch_cursor(&db, parent.id, 16, false).await;
        assert_eq!(
            background_tasks::Entity::find().count(&db).await.unwrap(),
            18
        );
        let child_type = if kind == "segments" {
            "regenerate_activity_segments"
        } else {
            "reprocess_activity_import"
        };
        assert_eq!(
            background_tasks::Entity::find()
                .filter(background_tasks::Column::TaskType.eq(child_type))
                .count(&db)
                .await
                .unwrap(),
            16
        );
        let continuation = background_tasks::Entity::find()
            .filter(background_tasks::Column::TaskType.eq(processor.task_type()))
            .filter(background_tasks::Column::Id.ne(parent.id))
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        PipelineContext::scope(
            Some(origin.for_task(continuation.id)),
            processor.process(continuation.id, continuation.payload),
        )
        .await
        .unwrap();
        assert_batch_cursor(&db, parent.id, 18, true).await;
        assert_eq!(
            background_tasks::Entity::find().count(&db).await.unwrap(),
            20
        );
    }
}

#[tokio::test]
async fn pipeline_registry_contains_every_default_processor() {
    let db = database().await;
    let registry = register_default_processors(TaskWorker::new(db.clone()), db)
        .await
        .unwrap()
        .registered_task_types();
    assert_eq!(registry.len(), 22); // Email is registered separately.
    for action in bike_core::strava::workflow::Action::ALL {
        assert!(registry.contains(&action.task_type().into()));
    }
    assert!(registry.contains(&"strava_workflow".into()));
    assert!(registry.contains(&"strava_reconcile".into()));
    assert!(registry.contains(&"strava_retention".into()));
    assert!(registry.contains(&"regenerate_activity_segments".into()));
}

#[tokio::test]
async fn pipeline_processors_report_missing_owned_source_as_a_retryable_failure() {
    let db = database().await;
    let workflow = LegacyWorkflow::new(db.clone());
    assert!(workflow
        .process(1, json!({"data":{"workflow_id":"missing"}}))
        .await
        .unwrap_err()
        .to_string()
        .contains("Strava workflow was not found"));
    let archive = ActivityArchiveImport::new(db);
    assert!(archive
        .process(1, json!({"data":{"job_id":999}}))
        .await
        .unwrap_err()
        .to_string()
        .contains("Archive job disappeared"));
}

#[tokio::test]
async fn pipeline_activity_match_processor_accumulates_affected_segments_in_its_batch() {
    let db = database().await;
    seed_activity(&db, 1).await;
    let parent = background_tasks::Model::enqueue(
        &db,
        "regenerate_user_segments".into(),
        json!({"data":{"user_id":7}}),
        None,
        3,
    )
    .await
    .unwrap();
    let batch = bike_core::background_jobs::batches::start(
        &db,
        parent.id,
        7,
        "segments",
        json!({"segment_ids":[],"caches_scheduled":false}),
    )
    .await
    .unwrap();
    let processor = RegenerateActivitySegments::new(db.clone());
    assert_eq!(processor.task_type(), "regenerate_activity_segments");
    bike_core::background_jobs::batches::scope(
        Some(batch.id),
        processor.process(2, json!({"data":{"activity_id":1}})),
    )
    .await
    .unwrap();
    let stored = worker_batches::Entity::find_by_id(batch.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.source["segment_ids"], json!([]));
}

#[tokio::test]
async fn pipeline_archive_redelivery_of_a_sealed_batch_does_not_spawn_more_children() {
    let db = database().await;
    let parent = background_tasks::Model::enqueue(
        &db,
        "activity_archive_import".into(),
        json!({"data":{"job_id":9}}),
        None,
        3,
    )
    .await
    .unwrap();
    let mut batch: worker_batches::ActiveModel =
        bike_core::background_jobs::batches::start(&db, parent.id, 7, "archive", json!({}))
            .await
            .unwrap()
            .into();
    batch.sealed = Set(true);
    batch.update(&db).await.unwrap();
    let processor = ActivityArchiveImport::new(db.clone());
    assert_eq!(processor.task_type(), "activity_archive_import");
    processor
        .process(parent.id, json!({"data":{"job_id":9,"batch_id":parent.id}}))
        .await
        .unwrap();
    assert_eq!(
        background_tasks::Entity::find().count(&db).await.unwrap(),
        1
    );
}
