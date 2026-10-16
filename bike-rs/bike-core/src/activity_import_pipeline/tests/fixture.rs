//! Retained-source fixtures for the legacy import boundary during retirement.
use super::*;
use crate::test_support as fixtures;

pub(super) fn state() -> ActivityProcessingState {
    ActivityProcessingState {
        attempt: activity_import_attempts::Model {
            status: "running".into(),
            ..fixtures::activity_import_attempt()
        },
        import_model: fixtures::reprocess_import(),
        activity_model: Some(fixtures::reprocess_activity()),
        parsed_activity: Some(fixtures::parsed_reprocess_activity()),
        affected_segment_ids: vec![fixtures::segment_effort().segment_id],
        parsing_artifact: activity_processing_artifact_from_model(fixtures::reprocess_artifact()),
        bytes: fixtures::REPROCESS_SOURCE.to_vec(),
    }
}

pub(super) fn checkpoint(state: &ActivityProcessingState) -> ProcessingCheckpoint {
    ProcessingCheckpoint {
        version: 1,
        parsed: state.parsed_activity.clone().unwrap(),
        affected_segment_ids: state.affected_segment_ids.clone(),
        activity_updated_at: state.activity_model.as_ref().map(|activity| activity.updated_at),
    }
}

pub(super) fn run(db: &DatabaseConnection) -> ActivityProcessingRun<'_> {
    ActivityProcessingRun {
        db,
        uploads_dir: "/unused",
        user_id: fixtures::customer().id,
        import: fixtures::reprocess_import(),
        existing_activity: Some(fixtures::reprocess_activity()),
        source_correlation_id: None,
        deduplication: ActivityUploadDeduplication::Disabled,
        training_profile: None,
    }
}

pub(super) struct RetainedSource {
    pub directory: tempfile::TempDir,
    pub artifact: activity_import_artifacts::Model,
    pub activity: activities::Model,
    pub attempt: activity_import_attempts::Model,
}

impl RetainedSource {
    pub async fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let objects = ObjectStore::filesystem(directory.path(), ObjectOwner::ActivityImports);
        let object = objects
            .write("activity-imports/fixtures", fixtures::REPROCESS_SOURCE)
            .await
            .unwrap();
        let artifact = activity_import_artifacts::Model {
            storage_path: object.path,
            ..fixtures::reprocess_artifact()
        };
        let state = state();
        let mut records = execution::stages(&state.attempt).unwrap();
        for record in &mut records {
            record.status = "completed".into();
        }
        let attempt = activity_import_attempts::Model {
            status: "completed".into(),
            source_json: stage_execution::artifact_metadata(&activity_processing_artifact_from_model(artifact.clone())),
            stages_json: serde_json::to_value(records).unwrap(),
            checkpoint_json: Some(serde_json::to_value(checkpoint(&state)).unwrap()),
            ..state.attempt
        };
        Self {
            directory,
            artifact,
            activity: fixtures::reprocess_activity(),
            attempt,
        }
    }

    pub fn database(&self) -> DatabaseConnection {
        fixtures::postgres_mock()
            .append_query_results([vec![self.artifact.clone()]])
            .append_query_results([vec![self.activity.clone()]])
            .append_query_results([vec![activity_import_attempts::Model {
                checkpoint_json: None,
                ..self.attempt.clone()
            }]])
            .append_query_results([vec![self.attempt.clone()]])
            .into_connection()
    }

    pub async fn plan(&self, db: &DatabaseConnection, stage: &str) -> ActivityImportReplayPlan {
        plan_activity_import_replay(
            db,
            self.directory.path().to_str().unwrap(),
            &fixtures::reprocess_import(),
            stage,
        )
        .await
        .unwrap()
    }
}
