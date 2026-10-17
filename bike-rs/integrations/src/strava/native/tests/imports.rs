use super::*;
use crate::strava::objects::ObjectRef;
use bike_core::{
    activity_import_pipeline::ActivityUploadPayload,
    entities::{activity_import_artifacts, activity_imports},
};

#[tokio::test]
async fn strava_source_creates_an_import_and_records_its_provider_artifact() {
    verify_source_save(false).await;
}

#[tokio::test]
async fn strava_source_updates_an_owned_import_and_preserves_its_id() {
    verify_source_save(true).await;
}

async fn verify_source_save(update: bool) {
    let fixture = Fixture::new();
    let import = import(&fixture);
    let upload = ActivityUploadPayload {
        original_filename: "fixture-strava.json".into(),
        format: "json".into(),
        mime_type: Some("application/json".into()),
        source_correlation_id: None,
        bytes: b"fixture provider data".to_vec(),
    };
    let object = ObjectRef {
        path: "fixture/strava.json".into(),
        bytes: upload.bytes.len(),
        sha256: "fixture-checksum".into(),
    };
    let mut mock = MockDatabase::new(DatabaseBackend::Postgres);
    if update {
        mock = mock.append_query_results([vec![import.clone()]]);
    }
    let db = mock
        .append_query_results([vec![import.clone()]])
        .append_query_results([Vec::<activity_import_artifacts::Model>::new()])
        .append_query_results([vec![artifact(&fixture)]])
        .into_connection();
    let saved = activity_imports::Entity::native_strava_source(
        &db,
        &fixture.event,
        update.then_some(import.id),
        &object,
        &upload,
    )
    .await
    .unwrap();
    assert_eq!(saved.id, import.id);
    let transactions = db.into_transaction_log();
    let statement = transactions
        .iter()
        .flat_map(|transaction| transaction.statements())
        .find(|statement| statement.sql.contains("activity_imports") && !statement.sql.starts_with("SELECT"))
        .unwrap();
    let expected = if update { "UPDATE" } else { "INSERT" };
    assert!(statement.sql.starts_with(expected));
    let values = &statement.values.as_ref().unwrap().0;
    assert!(values.contains(&"strava_sync".into()));
    assert!(values.contains(&"processing".into()));
    assert!(values.contains(&bike_core::activity_import_lifecycle::ACTIVITY_IMPORT_STAGE_ACTIVITY_SAVED.into()));
    assert!(values.contains(&object.path.clone().into()));
    assert!(transactions
        .iter()
        .flat_map(|transaction| transaction.statements())
        .any(|statement| statement.sql.starts_with("INSERT INTO \"activity_import_artifacts\"")));
}

pub(super) fn import(fixture: &Fixture) -> activity_imports::Model {
    activity_imports::Model {
        id: 18,
        user_id: fixture.customer.id,
        source: "strava_sync".into(),
        format: "json".into(),
        status: "processing".into(),
        processing_stage: "persist_activity".into(),
        last_processing_event_at: None,
        original_filename: "fixture-strava.json".into(),
        storage_path: "fixture/strava.json".into(),
        size_bytes: 21,
        mime_type: Some("application/json".into()),
        created_at: fixture.connection.created_at,
        updated_at: fixture.connection.created_at,
        ..bike_core::test_support::activity_import()
    }
}

pub(super) fn artifact(fixture: &Fixture) -> activity_import_artifacts::Model {
    activity_import_artifacts::Model {
        id: 19,
        activity_import_id: 18,
        user_id: fixture.customer.id,
        artifact_kind: "provider_payload".into(),
        source_quality: "strava_streams".into(),
        format: "json".into(),
        original_filename: "fixture-strava.json".into(),
        storage_path: "fixture/strava.json".into(),
        size_bytes: 21,
        mime_type: Some("application/json".into()),
        checksum_sha256: "fixture-checksum".into(),
        created_at: fixture.connection.created_at,
        updated_at: fixture.connection.created_at,
    }
}
