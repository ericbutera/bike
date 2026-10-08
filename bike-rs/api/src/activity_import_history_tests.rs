use super::*;
use bike_core::activity_import_execution::StageRecord;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, PaginatorTrait, Schema};
use serde_json::json;

async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(activity_imports::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(activity_import_attempts::Entity))
        .await
        .unwrap();
    db.execute(
        &schema.create_table_from_entity(bike_core::entities::activity_import_artifacts::Entity),
    )
    .await
    .unwrap();
    db.execute(&schema.create_table_from_entity(bike_core::background_jobs::durable::Entity))
        .await
        .unwrap();
    db
}

#[tokio::test]
async fn replay_previews_fallback_and_queues_only_the_reviewed_plan() {
    use bike_core::background_jobs::durable;
    let db = database().await;
    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("ride.gpx"), "<gpx/>").unwrap();
    let mut import = imported(1, 7);
    import.storage_path = "ride.gpx".into();
    let active: activity_imports::ActiveModel = import.into();
    active.insert(&db).await.unwrap();
    let request = || ReplayRequest {
        stage: "activity_saved".into(),
        expected_start_stage: None,
        expected_reused_attempt_id: None,
    };
    let uploads = source.path().to_str().unwrap();
    let plan = replay(&db, uploads, 7, 1, request(), false).await.unwrap();
    assert_eq!(plan.start_stage, "raw_stored");
    assert_eq!(plan.attempt_id, None);
    assert!(plan.reason.unwrap().contains("missing or stale"));
    assert_eq!(durable::Entity::find().count(&db).await.unwrap(), 0);
    let stale = ReplayRequest {
        expected_start_stage: Some("activity_saved".into()),
        ..request()
    };
    let error = replay(&db, uploads, 7, 1, stale, true).await.unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::CONFLICT);
    assert!(error.message.contains("prerequisites changed"));
    assert_eq!(durable::Entity::find().count(&db).await.unwrap(), 0);
    let reviewed = ReplayRequest {
        expected_start_stage: Some(plan.start_stage),
        expected_reused_attempt_id: plan.reused_attempt_id,
        ..request()
    };
    let result = replay(&db, uploads, 7, 1, reviewed, true).await.unwrap();
    let attempt_id = result.attempt_id.unwrap();
    let attempt = activity_import_attempts::Entity::find_by_id(attempt_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(attempt.status, "queued");
    assert_eq!(attempt.requested_stage, "activity_saved");
    assert_eq!(attempt.start_stage, "raw_stored");
    let tasks = durable::Entity::find().all(&db).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_type, "process_activity_import");
    assert_eq!(tasks[0].payload["data"]["attempt_id"], attempt_id);
}

fn imported(id: i32, user_id: i32) -> activity_imports::Model {
    let now = Utc::now();
    activity_imports::Model {
        id,
        user_id,
        import_version: 2,
        source: "manual_upload".into(),
        format: "gpx".into(),
        status: "failed".into(),
        activity_id: None,
        archive_job_id: None,
        processing_stage: "activity_parsed".into(),
        processing_error: Some("Invalid GPX".into()),
        processing_attempts: 1,
        processed_at: None,
        last_processing_event_at: Some(now),
        original_filename: "ride.gpx".into(),
        storage_path: "/private/ride.gpx".into(),
        size_bytes: 12,
        mime_type: None,
        created_at: now,
        updated_at: now,
    }
}

#[tokio::test]
async fn history_lists_only_owned_imports_even_without_an_activity() {
    let db = database().await;
    for import in [imported(1, 7), imported(2, 8)] {
        let active: activity_imports::ActiveModel = import.into();
        active.insert(&db).await.unwrap();
    }
    let result = history(
        &db,
        7,
        HistoryQuery {
            page: Some(0),
            source: Some("manual_upload".into()),
            status: Some("failed".into()),
            archive_job_id: None,
        },
    )
    .await
    .unwrap();
    assert_eq!((result.page, result.per_page, result.total), (1, 25, 1));
    assert_eq!(result.items[0].id, 1);
    assert_eq!(result.items[0].activity_id, None);
    assert_eq!(
        result.items[0].processing_error.as_deref(),
        Some("Invalid GPX")
    );
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("/private/"));
    let owned = owned_import_for_inspection(&db, 7, 1).await.unwrap();
    assert_eq!(owned.id, 1);
    assert!(owned_import_for_inspection(&db, 8, 1).await.is_err());
}

#[tokio::test]
async fn attempts_preserve_stage_summaries_without_exposing_private_source_or_checkpoint() {
    let db = database().await;
    let import = imported(1, 7);
    let now = Utc::now();
    let record = StageRecord {
        stage: "raw_stored".into(),
        status: "reused".into(),
        started_at: Some(now),
        completed_at: Some(now),
        summary: vec!["GPX · 12 bytes".into()],
        error: None,
        reused_attempt_id: Some(10),
    };
    let attempt = activity_import_attempts::Model {
        id: 11,
        user_id: 7,
        activity_import_id: 1,
        activity_id: None,
        status: "completed".into(),
        requested_stage: "activity_parsed".into(),
        start_stage: "activity_parsed".into(),
        current_stage: "complete".into(),
        reused_attempt_id: Some(10),
        source_json: json!({"filename": "ride.gpx", "storage_path": "/private/ride.gpx"}),
        stages_json: serde_json::to_value([record]).unwrap(),
        checkpoint_json: Some(json!({"private_checkpoint": true})),
        error: None,
        created_at: now,
        started_at: Some(now),
        finished_at: Some(now),
        updated_at: now,
    };
    let active: activity_import_attempts::ActiveModel = attempt.into();
    active.insert(&db).await.unwrap();
    let result = attempts(&db, &import).await.unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, 11);
    assert_eq!(result[0].source, json!({"filename": "ride.gpx"}));
    let node = result[0]
        .nodes
        .iter()
        .find(|node| node.stage == "raw_stored")
        .unwrap();
    assert_eq!(node.status, "reused");
    assert_eq!(node.summary, ["GPX · 12 bytes"]);
    assert_eq!(node.reused_attempt_id, Some(10));
    assert!(node.started_at.is_some() && node.completed_at.is_some());
    assert!(result[0].nodes.iter().any(|node| node.status == "unknown"));
    assert!(!serde_json::to_string(&result)
        .unwrap()
        .contains("private_checkpoint"));
}
