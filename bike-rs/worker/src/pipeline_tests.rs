use crate::{pipeline_fixture_tests::database, tasks::*};
use bike_core::{
    background_jobs::{
        entities::{background_tasks, pipeline_outputs, work_units, worker_batches},
        execution::ExecutionContext,
        pipeline::PipelineContext,
        worker::{TaskProcessor, TaskWorker},
    },
    entities::{
        activities, activity_imports, segment_summaries, segments, strava_delivery_intents,
        strava_gateway_watermarks,
    },
    strava_gateway_delivery::intent::{accept, DeliverySource},
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, Set};
use serde_json::json;

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
    assert_eq!(registry.len(), 14); // Email is registered separately.
    assert!(registry.contains(&"receive_strava_delivery".into()));
    assert!(registry.contains(&"regenerate_activity_segments".into()));
}

#[tokio::test]
async fn pipeline_processors_report_missing_owned_source_as_a_retryable_failure() {
    let db = database().await;
    let delivery = ReceiveStravaDelivery::new(db.clone());
    assert!(delivery
        .process(1, json!({"data":{"delivery_id":"missing"}}))
        .await
        .unwrap_err()
        .to_string()
        .contains("Delivery intent not found"));
    let archive = ActivityArchiveImport::new(db);
    assert!(archive
        .process(1, json!({"data":{"job_id":999}}))
        .await
        .unwrap_err()
        .to_string()
        .contains("Archive job disappeared"));
}

#[tokio::test]
async fn pipeline_obsolete_delivery_clears_owned_source_and_redelivery_is_idempotent() {
    let db = database().await;
    strava_gateway_watermarks::ActiveModel {
        athlete_id: Set(7),
        activity_id: Set(42),
        event_time: Set(2),
        operation: Set("delete".into()),
    }
    .insert(&db)
    .await
    .unwrap();
    let origin = PipelineContext::received("strava_webhook", Some("worker-receipt".into()));
    PipelineContext::scope(
        Some(origin.clone()),
        accept(
            &db,
            DeliverySource {
                delivery_id: "rust:42".into(),
                athlete_id: 7,
                user_id: 7,
                activity_id: 42,
                event_time: 1,
                operation: "delete".into(),
                payload: None,
            },
        ),
    )
    .await
    .unwrap();
    let processor = ReceiveStravaDelivery::new(db.clone());
    assert_eq!(processor.task_type(), "receive_strava_delivery");
    for _ in 0..2 {
        PipelineContext::scope(
            Some(origin.clone()),
            processor.process(1, json!({"data":{"delivery_id":"rust:42"}})),
        )
        .await
        .unwrap();
    }
    let intent = strava_delivery_intents::Entity::find_by_id("rust:42")
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert!(intent.source.is_none());
    assert!(intent.completed_at.is_some());
    assert_eq!(
        background_tasks::Entity::find().count(&db).await.unwrap(),
        1
    );
    assert!(processor.process(1, json!({"data":{}})).await.is_err());
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
