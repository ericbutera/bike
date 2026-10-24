//! Provider source adapters use the ordinary parser and owning saved-output models.
use crate::{
    activity_import_pipeline::ActivityUploadPayload,
    activity_parser::{parse_activity_artifact, ActivityParserArtifact},
    auth::entities::users,
    entities::{activities, strava_connections, strava_events},
    object_store::{ObjectOwner, ObjectStore},
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use strava_events::{ActivityChange, RetainedActivity};

pub(super) async fn process(
    db: &DatabaseConnection,
    objects: &ObjectStore,
    input: RetainedActivity,
) -> Result<ActivityChange, WorkflowError> {
    strava_connections::Model::for_event(db, &input.event).await?;
    input
        .event
        .validate_customer(&users::Model::find_by_id(db, input.event.bike_user_id).await?)?;
    if let Some(saved) = strava_events::Model::stored_mutation(db, &input.event).await? {
        return Ok(saved);
    }
    let bytes = objects.for_owner(ObjectOwner::Strava).read(&input.source).await?;
    let filename = format!("strava_activity_{}.json", input.event.activity_id()?);
    let parsed = parse_activity_artifact(ActivityParserArtifact {
        original_filename: &filename,
        format: "json",
        artifact_kind: "provider_payload",
        source_quality: "strava_streams",
        bytes: &bytes,
    })?;
    let payload: crate::activity_parser::strava::StoredStravaProviderPayload = serde_json::from_slice(&bytes)
        .map_err(|error| WorkflowError::invalid_data(format!("Invalid retained provider source: {error}")))?;
    if payload.activity.id != input.event.activity_id()? {
        return Err(WorkflowError::invalid_data(
            "Provider source does not match its activity event",
        ));
    }
    let profile = crate::training_profile::load_training_profile(db, input.event.bike_user_id).await?;
    let upload = ActivityUploadPayload {
        original_filename: filename,
        format: "json".into(),
        mime_type: Some("application/json".into()),
        source_correlation_id: Some(payload.activity.id.to_string()),
        bytes,
    };
    activities::Model::apply_strava_source(db, objects, input, &parsed, &profile, &upload).await
}
