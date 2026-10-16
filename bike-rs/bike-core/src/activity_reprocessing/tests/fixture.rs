use super::*;
use crate::activity_import_pipeline::ActivityProcessingArtifact;
use crate::entities::{
    activity_import_artifacts, activity_training_analyses, segment_efforts, segments,
    user_preferences,
};
use crate::object_store::ObjectOwner;
use ::object_store::{memory::InMemory, path::Path, ObjectStoreExt};
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, MockExecResult};
use std::sync::Arc;

pub fn mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
}

pub fn metadata(mock: MockDatabase) -> MockDatabase {
    mock.append_query_results([vec![fixtures::reprocess_activity()]])
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([vec![fixtures::reprocess_artifact()]])
}

pub fn saved_activity() -> activities::Model {
    let parsed = fixtures::parsed_reprocess_activity();
    activities::Model {
        title: parsed.draft.title,
        started_at: parsed.draft.started_at,
        updated_at: fixtures::timestamp() + chrono::Duration::seconds(1),
        ..fixtures::reprocess_activity()
    }
}

pub fn scope() -> RefreshScope {
    RefreshScope {
        revision: revision(saved_activity().updated_at).unwrap(),
        dirty_from: saved_activity().started_at.date_naive(),
        segment_ids: vec![fixtures::segment_effort().segment_id],
        heatmap_generation: Some(fixtures::heatmap_projection().generation as u64),
    }
}

pub fn history() -> activity_event_history::Model {
    let mut event = activity_event_history::Event::import(
        &fixtures::reprocess_execution(),
        Some(&fixtures::reprocess_activity()),
        &saved_activity(),
    )
    .unwrap();
    event.context["refresh_scope"] = serde_json::to_value(scope()).unwrap();
    activity_event_history::Model {
        id: event.id,
        source: event.source,
        context_json: event.context,
        ..fixtures::activity_history()
    }
}

pub fn source_reads(mock: MockDatabase) -> MockDatabase {
    metadata(mock.append_query_results([Vec::<activity_event_history::Model>::new()]))
        .append_query_results([vec![fixtures::reprocess_artifact()]]) // recording enrichment
        .append_query_results([Vec::<user_preferences::Model>::new()])
}

/// Existing domain reads/writes for one ride with an old segment and no new matches.
pub fn mutation(mock: MockDatabase, history_write: Result<u64, DbErr>) -> MockDatabase {
    let state = fixtures::analytics_user_state();
    let mock = mock
        .append_query_results([vec![fixtures::reprocess_activity()]]) // row lock
        .append_query_results([Vec::<activity_event_history::Model>::new()])
        .append_query_results([vec![saved_activity()]]) // conditional source write
        .append_query_results([vec![std::collections::BTreeMap::from([(
            "segment_id",
            sea_orm::Value::from(fixtures::segment_effort().segment_id),
        )])]]) // single-column tuple read
        .append_query_results([Vec::<segments::Model>::new()]) // route candidates
        .append_query_results([Vec::<segment_efforts::Model>::new()]) // new membership
        .append_query_results([Vec::<segment_efforts::Model>::new()]) // achievements
        .append_query_results([Vec::<segment_efforts::Model>::new()]) // training context
        .append_query_results([Vec::<activity_training_analyses::Model>::new()])
        .append_query_results([vec![saved_activity()]])
        .append_query_results([vec![fixtures::training_analysis()]])
        .append_query_results([vec![state.clone()]]) // fitness dirty read
        .append_query_results([vec![state]]) // fitness dirty write
        .append_query_results([vec![fixtures::heatmap_projection()]])
        // efforts, old analytics, fitness requirement, segment change, segment requirement,
        // import status; nested transactions use savepoints rather than these exec results.
        .append_exec_results((0..6).map(|_| success()));
    match history_write {
        Err(error) => mock.append_exec_errors([error]),
        Ok(rows_affected) => mock.append_exec_results([MockExecResult {
            rows_affected,
            ..success()
        }]),
    }
}

fn success() -> MockExecResult {
    MockExecResult {
        last_insert_id: 0,
        rows_affected: 1,
    }
}

pub async fn retain_source(bytes: &'static [u8]) -> (ObjectStore, Arc<InMemory>) {
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    backend
        .put(
            &Path::parse(fixtures::reprocess_artifact().storage_path).unwrap(),
            bytes.into(),
        )
        .await
        .unwrap();
    (objects, backend)
}

pub fn artifact() -> ActivityProcessingArtifact {
    let source: activity_import_artifacts::Model = fixtures::reprocess_artifact();
    ActivityProcessingArtifact {
        size_bytes: source.size_bytes,
        checksum_sha256: Some(source.checksum_sha256),
        artifact_kind: source.artifact_kind,
        original_filename: source.original_filename,
        format: source.format,
        source_quality: source.source_quality,
        storage_path: source.storage_path,
    }
}

pub async fn apply(db: &DatabaseConnection) -> Result<RefreshScope, WorkflowError> {
    fixtures::reprocess_activity()
        .apply_reprocessing(
            db,
            &fixtures::reprocess_execution(),
            &artifact(),
            &fixtures::parsed_reprocess_activity(),
            &crate::training_profile::TrainingProfile::default(),
        )
        .await
}
