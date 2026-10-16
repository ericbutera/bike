//! Native reprocessing uses retained inputs and domain transactions, without SQL jobs.
use crate::{
    activity_import_pipeline::load_best_activity_parsing_artifact,
    activity_parser::{parse_activity_artifact, ActivityParserArtifact},
    bike_tasks::{
        ActivityImportResult, FitnessChange, HeatmapChange, ImportExecution, ImportSource,
        ImportTarget, Priority, ReprocessInput, SegmentChange,
    },
    entities::{activities, activity_event_history, activity_imports},
    object_store::ObjectStore,
    workflow_error::WorkflowError,
};
use chrono::NaiveDate;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

/// Small domain consequences retained with the existing audit event. In particular,
/// removed segment memberships cannot be reconstructed from the replacement rows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshScope {
    pub revision: u64,
    pub dirty_from: NaiveDate,
    pub segment_ids: Vec<i32>,
    pub heatmap_generation: Option<u64>,
}

impl RefreshScope {
    pub fn result(
        &self,
        command: &crate::bike_tasks::ActivityImport,
    ) -> Result<ActivityImportResult, WorkflowError> {
        let ImportTarget::ExistingActivity { activity_id, .. } = command.target else {
            return Err(WorkflowError::bad_request(
                "This adapter reprocesses existing activities",
            ));
        };
        let source = &command.source;
        Ok(ActivityImportResult {
            activity_id,
            activity_import_id: source.activity_import_id,
            fitness: FitnessChange {
                user_id: source.user_id,
                revision: self.revision,
                dirty_from: self.dirty_from,
                priority: source.priority,
            },
            segments: self
                .segment_ids
                .iter()
                .map(|segment_id| SegmentChange {
                    segment_id: *segment_id,
                    revision: self.revision,
                    priority: source.priority,
                })
                .collect(),
            heatmap: self.heatmap_generation.map(|generation| HeatmapChange {
                user_id: source.user_id,
                activity_id,
                generation,
                priority: source.priority,
            }),
        })
    }
}

/// Reads metadata only. Retained bytes are verified and parsed by the worker.
pub async fn command(
    db: &DatabaseConnection,
    activity_id: i32,
    command_id: String,
    priority: Priority,
) -> Result<ReprocessInput, WorkflowError> {
    let activity = activities::Model::find_by_id(db, activity_id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Activity not found"))?;
    let import = owned_import(db, &activity).await?;
    let artifact = load_best_activity_parsing_artifact(db, &import).await?;
    Ok(ReprocessInput {
        source: ImportSource {
            command_id,
            user_id: activity.user_id,
            activity_import_id: import.id,
            object: artifact.object_reference()?,
            priority,
            cause: None,
        },
        activity_id,
        expected_revision: revision(activity.updated_at)?,
    })
}

async fn owned_import(
    db: &DatabaseConnection,
    activity: &activities::Model,
) -> Result<activity_imports::Model, WorkflowError> {
    let id = activity
        .activity_import_id
        .ok_or_else(|| WorkflowError::bad_request("Activity is not linked to a retained import"))?;
    let import = activity_imports::Entity::find_owned(db, activity.user_id, id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Activity import not found"))?;
    import.require_sql_processing()?;
    if import.activity_id != Some(activity.id) {
        return Err(WorkflowError::conflict(
            "Import does not belong to this activity",
        ));
    }
    Ok(import)
}

pub fn revision(at: chrono::DateTime<chrono::Utc>) -> Result<u64, WorkflowError> {
    u64::try_from(at.timestamp_micros())
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| WorkflowError::bad_request("Activity requires a positive version timestamp"))
}

pub async fn reprocess(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    execution: ImportExecution,
) -> Result<ActivityImportResult, WorkflowError> {
    execution.command.validate()?;
    let ImportTarget::ExistingActivity { activity_id, .. } = execution.command.target else {
        return Err(WorkflowError::bad_request(
            "New imports have not been converted",
        ));
    };
    if let Some(scope) = activity_event_history::Model::import_refresh(db, &execution).await? {
        return scope.result(&execution.command);
    }
    let source = &execution.command.source;
    let activity = activities::Model::find_owned(db, activity_id, source.user_id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Activity not found"))?;
    let import = owned_import(db, &activity).await?;
    let artifact = load_best_activity_parsing_artifact(db, &import).await?;
    if import.id != source.activity_import_id || artifact.object_reference()? != source.object {
        return Err(WorkflowError::conflict(
            "Retained source changed since submission",
        ));
    }
    let bytes = objects.read(&source.object).await?;
    let mut parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &artifact.original_filename,
        format: &artifact.format,
        artifact_kind: &artifact.artifact_kind,
        source_quality: &artifact.source_quality,
        bytes: &bytes,
    })?;
    crate::activity_import_pipeline::enrich_recording(
        db,
        objects,
        &import,
        &artifact,
        Some(&activity),
        &mut parsed,
    )
    .await?;
    let profile = crate::training_profile::load_training_profile(db, source.user_id).await?;
    activity
        .apply_reprocessing(db, &execution, &artifact, &parsed, &profile)
        .await?
        .result(&execution.command)
}
