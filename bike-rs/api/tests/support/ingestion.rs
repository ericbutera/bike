use super::*;
use bike_core::background_jobs::worker::TaskWorker;
use bike_core::entities::activity_imports;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use std::sync::Arc;
use std::time::Duration;
use worker::tasks::{ProcessActivityImport, RebuildFitnessFreshness};

async fn upload(app: &Router, bytes: &[u8]) -> Value {
    let boundary = "bike-ingestion-test-boundary";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"synthetic.gpx\"\r\nContent-Type: application/gpx+xml\r\n\r\n").into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/activity-imports")
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 202);
    serde_json::from_slice(
        &axum::body::to_bytes(response.into_body(), 1_048_576)
            .await
            .unwrap(),
    )
    .unwrap()
}

async fn await_outcome(app: &Router, id: i64, outcome: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let trace = get_json(app, &format!("/api/activity-imports/{id}/trace")).await;
            if trace["import"]["status"] == outcome {
                return trace;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("worker completes ingestion")
}

fn start_worker(db: sea_orm::DatabaseConnection, dir: String) -> tokio::task::JoinHandle<()> {
    tokio::spawn(
        TaskWorker::new(db.clone())
            .with_poll_interval(Duration::from_millis(20))
            .register_processor(Arc::new(ProcessActivityImport::with_uploads_dir(
                db.clone(),
                dir,
            )))
            .register_processor(Arc::new(RebuildFitnessFreshness::new(db)))
            .run(),
    )
}

#[tokio::test]
async fn owner_upload_worker_trace_and_replay_run_to_completion() {
    let dir = std::env::temp_dir().join(format!("bike-ingestion-http-{}", uuid::Uuid::new_v4()));
    let (app, db) = support::ingestion_platform_app(true, dir.display().to_string()).await;
    bike_core::auth::entities::users::ActiveModel {
        id: Set(1),
        is_admin: Set(Some(false)),
        ..Default::default()
    }
    .update(&db)
    .await
    .unwrap();
    let received = upload(
        &app,
        include_bytes!("../fixtures/import-analytics-ride.gpx"),
    )
    .await;
    let id = received["id"].as_i64().unwrap();
    let queued = get_json(&app, &format!("/api/activity-imports/{id}/trace")).await;
    assert_eq!(queued["attempts"][0]["status"], "queued");
    assert!(queued["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["status"] == "pending"));
    let worker = start_worker(db.clone(), dir.display().to_string());
    let first = await_outcome(&app, id, "processed").await;
    assert_eq!(first["nodes"][1]["summary"][0], "Parsed 7 records");
    assert!(first["nodes"][0]["summary"][0]
        .as_str()
        .unwrap()
        .contains("bytes"));
    assert!(first["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|node| node["status"] == "completed"));
    let plan = get_json(
        &app,
        &format!("/api/activity-imports/{id}/replay?stage=activity_saved"),
    )
    .await;
    assert_eq!(plan["start_stage"], "activity_saved");
    request_json(
        &app,
        "POST",
        &format!("/api/activity-imports/{id}/replay"),
        Some(&serde_json::json!({"stage":"activity_saved"})),
        202,
    )
    .await;
    let replayed = await_outcome(&app, id, "processed").await;
    assert_eq!(
        replayed["import"]["activity_id"],
        first["import"]["activity_id"]
    );
    assert_eq!(replayed["attempts"].as_array().unwrap().len(), 2);
    assert_eq!(replayed["attempts"][1], first["attempts"][0]);
    assert_eq!(replayed["nodes"][0]["status"], "reused");
    assert_eq!(replayed["nodes"][1]["status"], "reused");
    assert_eq!(replayed["nodes"][6]["status"], "completed");
    worker.abort();
    let _ = worker.await;
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn history_is_paginated_owner_scoped_and_includes_failed_unlinked_imports() {
    let (app, db) = support::ingestion_platform_app(true, String::new()).await;
    for index in 0..27 {
        activity_imports::ActiveModel {
            user_id: Set(1),
            import_version: Set(2),
            source: Set("archive_url_import".into()),
            format: Set("gpx".into()),
            status: Set("failed".into()),
            processing_attempts: Set(1),
            processing_stage: Set("activity_parsed".into()),
            processing_error: Set(Some("Parse failed".into())),
            original_filename: Set(format!("entry-{index}.gpx")),
            storage_path: Set(format!("entry-{index}.gpx")),
            size_bytes: Set(10),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
    }
    let first = get_json(
        &app,
        "/api/activity-imports/history?source=archive_url_import&status=failed&page=1",
    )
    .await;
    let second = get_json(
        &app,
        "/api/activity-imports/history?source=archive_url_import&status=failed&page=2",
    )
    .await;
    assert_eq!(first["total"], 27);
    assert_eq!(first["items"].as_array().unwrap().len(), 25);
    assert_eq!(second["items"].as_array().unwrap().len(), 2);
    assert!(first["items"][0]["activity_id"].is_null());
    let id = first["items"][0]["id"].as_i64().unwrap() as i32;
    let mut other = activity_imports::Entity::find_by_id(id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    other.user_id = 999;
    let mut active: activity_imports::ActiveModel = other.into();
    active.user_id = Set(999);
    active.update(&db).await.unwrap();
    request_json(
        &app,
        "GET",
        &format!("/api/activity-imports/{id}/replay?stage=raw_stored"),
        None,
        404,
    )
    .await;
    request_json(
        &app,
        "POST",
        &format!("/api/activity-imports/{id}/replay"),
        Some(&serde_json::json!({"stage":"raw_stored"})),
        404,
    )
    .await;
    let filtered = get_json(
        &app,
        "/api/activity-imports/history?source=archive_url_import&status=failed&page=1",
    )
    .await;
    assert_eq!(filtered["total"], 26);
}
