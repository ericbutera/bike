use super::*;
use crate::{
    entities::activity_import_artifacts,
    strava_objects::{checksum, ObjectRef},
};
use sea_orm::{ActiveModelTrait, Database, EntityTrait, Schema, Set};
use serde_json::json;
use std::{path::Path, time::SystemTime};

async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    for table in [
        schema.create_table_from_entity(activity_imports::Entity),
        schema.create_table_from_entity(activity_import_artifacts::Entity),
        schema.create_table_from_entity(strava_workflows::Entity),
        schema.create_table_from_entity(integration_events::Entity),
        schema.create_table_from_entity(crate::background_jobs::background_tasks::Entity),
    ] {
        db.execute(&table).await.unwrap();
    }
    db
}

async fn workflow(
    db: &DatabaseConnection,
    id: &str,
    parent: Option<&str>,
    object: &ObjectRef,
    completed: Option<DateTime<Utc>>,
) {
    strava_workflows::ActiveModel {
        id: Set(id.into()),
        connection_id: Set(1),
        athlete_id: Set(99),
        activity_id: Set(None),
        action: Set("backfill".into()),
        source: Set(json!({"parent":parent})),
        checkpoint: Set(json!({"discover":object})),
        status: Set(if completed.is_some() {
            "completed"
        } else {
            "running"
        }
        .into()),
        accepted_at: Set(Utc::now() - Duration::days(60)),
        completed_at: Set(completed),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
}

fn age(path: &Path, at: DateTime<Utc>) {
    std::fs::File::open(path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(SystemTime::from(at)))
        .unwrap();
}

async fn age_objects(root: &Path, at: DateTime<Utc>) {
    for file in files::inventory(root.to_str().unwrap(), &BTreeSet::new())
        .await
        .unwrap()
    {
        age(&root.join(file.path), at);
    }
}

async fn source_artifacts(db: &DatabaseConnection, root: &Path) -> Vec<String> {
    let paths = [
        "activity-imports/test/current.json",
        "activity-imports/test/original.fit",
        "activity-imports/test/original.tcx",
        "activity-imports/test/original.gpx",
    ];
    tokio::fs::create_dir_all(root.join("activity-imports/test"))
        .await
        .unwrap();
    activity_imports::ActiveModel {
        id: Set(7),
        user_id: Set(1),
        import_version: Set(1),
        source: Set("strava_sync".into()),
        format: Set("fit".into()),
        status: Set("processed".into()),
        processing_stage: Set("completed".into()),
        processing_attempts: Set(1),
        original_filename: Set("original.fit".into()),
        storage_path: Set(paths[1].into()),
        size_bytes: Set(2),
        ..Default::default()
    }
    .insert(db)
    .await
    .unwrap();
    for (index, path) in paths.iter().enumerate() {
        tokio::fs::write(root.join(path), b"{}").await.unwrap();
        activity_import_artifacts::ActiveModel {
            activity_import_id: Set(7),
            user_id: Set(1),
            artifact_kind: Set(if index == 0 {
                "provider_payload"
            } else {
                "original"
            }
            .into()),
            format: Set(path.rsplit('.').next().unwrap().into()),
            source_quality: Set(if index == 0 {
                "strava_streams"
            } else {
                "original"
            }
            .into()),
            original_filename: Set(path.to_string()),
            storage_path: Set(path.to_string()),
            size_bytes: Set(2),
            checksum_sha256: Set(checksum(b"{}")),
            ..Default::default()
        }
        .insert(db)
        .await
        .unwrap();
    }
    paths.iter().map(|path| path.to_string()).collect()
}

async fn event(db: &DatabaseConnection, workflow: &str) {
    integration_events::Entity::record(
        db,
        integration_events::NewEvent {
            user_id: Some(1),
            connection_id: Some(1),
            provider: "strava".into(),
            event_type: "workflow.stage".into(),
            level: "info".into(),
            message: "Fixture".into(),
            payload: Some(json!({"workflow_id":workflow})),
        },
    )
    .await
    .unwrap();
}

async fn assert_source_files_preserved(root: &Path, sources: &[String]) {
    for path in sources {
        assert_eq!(tokio::fs::read(root.join(path)).await.unwrap(), b"{}");
    }
}

#[tokio::test]
async fn strava_retention_preserves_sources_active_trees_and_recent_dependencies() {
    let db = database().await;
    let root = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(root.path());
    let now = Utc::now();
    let old = now - Duration::days(31);
    let sources = source_artifacts(&db, root.path()).await;
    let raw = store
        .save_response(
            "strava/99/runs/child/raw/pages/1",
            200,
            Default::default(),
            b"[]",
        )
        .await
        .unwrap();
    let discovered = store
        .transform(
            "strava/99/runs/parent/transforms/discover",
            &json!([{"raw":raw.body}]),
        )
        .await
        .unwrap();
    workflow(&db, "parent", None, &discovered, None).await;
    workflow(&db, "child", Some("parent"), &raw.body, Some(old)).await;
    let recent = store
        .write("strava/99/runs/recent/raw/pages/1", b"[]")
        .await
        .unwrap();
    workflow(&db, "recent", None, &recent, Some(now)).await;
    let expired = store
        .save_response(
            "strava/99/runs/expired/raw/pages/1",
            200,
            Default::default(),
            b"[]",
        )
        .await
        .unwrap();
    workflow(&db, "expired", None, &expired.body, Some(old)).await;
    event(&db, "expired").await;
    event(&db, "child").await;
    let unrelated = root.path().join("activity-imports/test/other.json");
    let superseded = root
        .path()
        .join("activity-imports/test/strava_previous.json");
    tokio::fs::write(&unrelated, b"{}").await.unwrap();
    tokio::fs::write(&superseded, b"{}").await.unwrap();
    age(&unrelated, old);
    age_objects(root.path(), old).await;
    let boundary = store
        .write("strava/99/runs/boundary/raw", b"{}")
        .await
        .unwrap();
    age(
        &root.path().join(&boundary.path),
        now - Duration::days(TTL_DAYS),
    );

    let result = run(&db, root.path().to_str().unwrap(), now).await.unwrap();
    assert_eq!(result.files, 4); // Expired response's 3 files + superseded payload.
    assert_eq!(result.workflows, 1);
    assert_eq!(result.events, 1);
    assert!(!superseded.exists());
    assert!(!root.path().join(&expired.body.path).exists());
    assert!(unrelated.exists());
    assert_source_files_preserved(root.path(), &sources).await;
    assert!(store
        .response("strava/99/runs/child/raw/pages/1")
        .await
        .unwrap()
        .is_some());
    assert_eq!(store.read(&recent).await.unwrap(), b"[]");
    assert!(root.path().join(&boundary.path).exists());
    let repeated = run(&db, root.path().to_str().unwrap(), now).await.unwrap();
    assert_eq!(repeated.files, 0);
    assert_eq!(repeated.workflows, 0);
    assert_eq!(repeated.events, 0);
}

#[tokio::test]
async fn strava_retention_verifies_protection_before_any_deletion() {
    let db = database().await;
    let root = tempfile::tempdir().unwrap();
    let store = ObjectStore::new(root.path());
    let object = store
        .write("strava/99/runs/active/raw", b"{}")
        .await
        .unwrap();
    workflow(&db, "active", None, &object, None).await;
    let expired = store
        .write("strava/99/old/transforms/stage", b"{}")
        .await
        .unwrap();
    age_objects(root.path(), Utc::now() - Duration::days(31)).await;
    tokio::fs::write(root.path().join(&object.path), b"bad")
        .await
        .unwrap();
    assert!(run(&db, root.path().to_str().unwrap(), Utc::now())
        .await
        .is_err());
    assert!(root.path().join(&expired.path).exists());
    assert!(strava_workflows::Entity::find_by_id("active")
        .one(&db)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn strava_retention_keeps_fallback_objects_when_current_source_is_corrupt() {
    let db = database().await;
    let root = tempfile::tempdir().unwrap();
    let sources = source_artifacts(&db, root.path()).await;
    let source = activity_import_artifacts::Entity::strava_payload(&db, 1, 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        source.storage_path, sources[0],
        "JSON remains available with FIT as primary"
    );
    let fallback = ObjectStore::new(root.path())
        .write("strava/99/old/raw/activity", b"{}")
        .await
        .unwrap();
    age(
        &root.path().join(&fallback.path),
        Utc::now() - Duration::days(31),
    );
    tokio::fs::write(root.path().join(&source.storage_path), b"corrupt")
        .await
        .unwrap();
    let error = run(&db, root.path().to_str().unwrap(), Utc::now())
        .await
        .unwrap_err();
    assert!(error.message.contains("failed checksum verification"));
    assert!(root.path().join(&fallback.path).exists());
}

#[cfg(unix)]
#[tokio::test]
async fn strava_retention_refuses_symlinks_and_preserves_external_files() {
    let db = database().await;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let external = outside.path().join("original.fit");
    tokio::fs::write(&external, b"original").await.unwrap();
    tokio::fs::create_dir(root.path().join("strava"))
        .await
        .unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("strava/escape")).unwrap();
    assert!(run(&db, root.path().to_str().unwrap(), Utc::now())
        .await
        .is_err());
    assert_eq!(tokio::fs::read(external).await.unwrap(), b"original");
}

#[tokio::test]
async fn strava_retention_waits_for_queue_ack_before_expiring_completed_work() {
    use crate::background_jobs::background_tasks;
    let db = database().await;
    let root = tempfile::tempdir().unwrap();
    let old = Utc::now() - Duration::days(31);
    let object = ObjectStore::new(root.path())
        .write("strava/99/runs/finished/raw", b"[]")
        .await
        .unwrap();
    age(&root.path().join(&object.path), old);
    workflow(&db, "finished", None, &object, Some(old)).await;
    let job = background_tasks::ActiveModel {
        task_type: Set("strava_workflow".into()),
        payload: Set(json!({"data":{"workflow_id":"finished"}})),
        status: Set("pending".into()),
        attempts: Set(1),
        max_attempts: Set(3),
        created_at: Set(old),
        updated_at: Set(old),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    let result = run(&db, root.path().to_str().unwrap(), Utc::now())
        .await
        .unwrap();
    assert_eq!(result.files, 0);
    assert_eq!(result.workflows, 0);
    let mut acknowledged: background_tasks::ActiveModel = job.into();
    acknowledged.status = Set("completed".into());
    acknowledged.update(&db).await.unwrap();
    let result = run(&db, root.path().to_str().unwrap(), Utc::now())
        .await
        .unwrap();
    assert_eq!(result.files, 1);
    assert_eq!(result.workflows, 1);
    assert!(!root.path().join(object.path).exists());
}
