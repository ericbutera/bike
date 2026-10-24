//! Read a retained file, parse it, and save its activity. Derived work is called by the workflow.
use crate::{
    activity_import_pipeline::load_best_activity_parsing_artifact,
    activity_parser::{parse_activity_artifact, ActivityParserArtifact},
    bike_tasks::{ActivityImportResult, ImportExecution, ImportSource, ImportTarget, Priority, ReprocessInput},
    entities::{activities, activity_imports},
    object_store::ObjectStore,
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
mod strava;

pub async fn process(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    execution: crate::bike_tasks::ActivityProcessingExecution,
) -> Result<crate::bike_tasks::ActivityProcessingResult, WorkflowError> {
    use crate::bike_tasks::{ActivityProcessingResult, ActivityProcessingSource};
    match execution.source {
        ActivityProcessingSource::Import(command) => {
            let result = reprocess(
                db,
                objects,
                ImportExecution {
                    command: *command,
                    workflow: execution.workflow,
                },
            )
            .await?;
            Ok(ActivityProcessingResult::Import(result))
        }
        ActivityProcessingSource::Strava(source) => {
            let result = strava::process(db, objects, *source).await?;
            Ok(ActivityProcessingResult::Strava(Box::new(result)))
        }
    }
}

pub async fn process_strava_source(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    source: crate::entities::strava_events::RetainedActivity,
) -> Result<crate::entities::strava_events::ActivityChange, WorkflowError> {
    strava::process(db, objects, source).await
}

#[cfg(test)]
mod tests;

pub async fn import_source(
    db: &DatabaseConnection,
    user_id: i32,
    import_id: i32,
    command_id: String,
    priority: Priority,
) -> Result<ImportSource, WorkflowError> {
    let import = activity_imports::Entity::find_owned(db, user_id, import_id)
        .await?
        .ok_or_else(|| WorkflowError::not_found("Activity import not found"))?;
    import.require_retained_file_processing()?;
    let artifact = load_best_activity_parsing_artifact(db, &import).await?;
    Ok(ImportSource {
        command_id,
        user_id,
        activity_import_id: import_id,
        object: artifact.object_reference()?,
        priority,
        cause: None,
    })
}

/// Submission reads owned metadata; parsing belongs to the worker.
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
    import.require_retained_file_processing()?;
    if import.activity_id != Some(activity.id) {
        return Err(WorkflowError::conflict("Import does not belong to this activity"));
    }
    Ok(import)
}

pub async fn reprocess(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    execution: ImportExecution,
) -> Result<ActivityImportResult, WorkflowError> {
    execution.command.validate()?;
    let source = &execution.command.source;
    let current = match execution.command.target {
        ImportTarget::ExistingActivity { activity_id } => Some(
            activities::Model::find_owned(db, activity_id, source.user_id)
                .await?
                .ok_or_else(|| WorkflowError::not_found("Activity not found"))?,
        ),
        ImportTarget::NewActivity => None,
    };
    let import = match &current {
        Some(activity) => owned_import(db, activity).await?,
        None => activity_imports::Entity::find_owned(db, source.user_id, source.activity_import_id)
            .await?
            .ok_or_else(|| WorkflowError::not_found("Activity import not found"))?,
    };
    import.require_retained_file_processing()?;
    let artifact = load_best_activity_parsing_artifact(db, &import).await?;
    if import.id != source.activity_import_id || artifact.object_reference()? != source.object {
        return Err(WorkflowError::conflict("Retained source changed since submission"));
    }
    let bytes = artifact.read(objects).await?;
    let mut parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &artifact.original_filename,
        format: &artifact.format,
        artifact_kind: &artifact.artifact_kind,
        source_quality: &artifact.source_quality,
        bytes: &bytes,
    })?;
    crate::activity_import_pipeline::enrich_recording(db, objects, &import, &artifact, current.as_ref(), &mut parsed)
        .await?;
    let profile = crate::training_profile::load_training_profile(db, source.user_id).await?;
    let activity = activities::Model::apply_import(db, objects, &execution, &artifact, &parsed, &profile).await?;
    Ok(ActivityImportResult {
        activity_id: activity.id,
        activity_import_id: import.id,
        user_id: activity.user_id,
        priority: source.priority,
    })
}
