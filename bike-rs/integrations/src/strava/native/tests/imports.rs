use super::Fixture;
use bike_core::entities::{activity_import_artifacts, activity_imports};

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
