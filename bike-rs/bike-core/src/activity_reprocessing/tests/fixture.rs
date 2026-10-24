use super::*;
use crate::{
    activity_import_pipeline::ActivityProcessingArtifact,
    entities::{activity_import_artifacts, user_preferences},
};
use ::object_store::{memory::InMemory, path::Path, ObjectStoreExt};
use sea_orm::{DatabaseBackend, DbErr, MockDatabase, MockExecResult};
use std::sync::Arc;

pub fn mock() -> MockDatabase {
    MockDatabase::new(DatabaseBackend::Postgres)
}

pub fn source_metadata(mock: MockDatabase, artifact: activity_import_artifacts::Model) -> MockDatabase {
    mock.append_query_results([vec![fixtures::reprocess_activity()]])
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([vec![artifact]])
}

pub fn metadata(mock: MockDatabase) -> MockDatabase {
    source_metadata(mock, fixtures::reprocess_artifact())
}

pub fn source_reads(mock: MockDatabase) -> MockDatabase {
    metadata(mock)
        .append_query_results([vec![fixtures::reprocess_artifact()]])
        .append_query_results([Vec::<user_preferences::Model>::new()])
}

pub fn saved_activity() -> activities::Model {
    let parsed = fixtures::parsed_reprocess_activity();
    activities::Model {
        derived_data_json: Some(crate::activity_data::StoredActivityDerivedData::from(
            &parsed.derived_data,
        )),
        title: parsed.draft.title,
        started_at: parsed.draft.started_at,
        updated_at: fixtures::timestamp() + chrono::Duration::seconds(1),
        ..fixtures::reprocess_activity()
    }
}

pub fn mutation(mock: MockDatabase, history_write: Result<u64, DbErr>) -> MockDatabase {
    let mock = mock
        .append_query_results([vec![fixtures::reprocess_import()]])
        .append_query_results([vec![fixtures::reprocess_activity()]])
        .append_query_results([vec![saved_activity()]])
        .append_query_results([Vec::<crate::entities::activity_recordings::Model>::new()])
        .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()]);
    match history_write {
        Err(error) => mock.append_exec_errors([error]),
        Ok(rows_affected) => mock.append_exec_results([MockExecResult {
            rows_affected,
            ..fixtures::written()
        }]),
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
    let source = fixtures::reprocess_artifact();
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

pub async fn apply(db: &DatabaseConnection) -> Result<activities::Model, WorkflowError> {
    activities::Model::apply_import(
        db,
        &objects(),
        &fixtures::reprocess_execution(),
        &artifact(),
        &fixtures::parsed_reprocess_activity(),
        &crate::training_profile::TrainingProfile::default(),
    )
    .await
}

pub fn objects() -> ObjectStore {
    fixtures::object_storage(ObjectOwner::BikeTasks).0
}
