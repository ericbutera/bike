use crate::activity_import_pipeline::{
    persist_activity_upload_with_artifacts, reprocess_activity_from_import,
    ActivityUploadDeduplication, ActivityUploadPayload, PersistActivityUploadOutcome,
    PersistActivityUploadWithArtifactsRequest, ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD,
    ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS,
};
use crate::auth::entities::users;
use crate::entities::{activities, activity_import_artifacts, activity_imports};
use crate::strava::build_activity_upload;
use crate::strava::provider_payload::{StravaActivityStreams, StravaActivitySummary};
use crate::strava::workflow::rebuild::Rebuild;
use crate::training_profile::load_training_profile;
use crate::workflow_error::WorkflowError as AppError;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use serde::Deserialize;

#[derive(Debug, serde::Serialize, Deserialize)]
pub struct ActivityPayload {
    pub activity: StravaActivitySummary,
    #[serde(default)]
    pub streams: StravaActivityStreams,
}

pub(crate) async fn upsert_activity(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    payload: &ActivityPayload,
) -> Result<Rebuild, AppError> {
    let user = users::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Bike user for Strava activity was not found"))?;
    let profile = load_training_profile(db, user_id).await?;
    let import_payload = build_activity_upload(&payload.activity, &payload.streams)?;
    let existing = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Source.eq("strava_sync"))
        .filter(activities::Column::SourceCorrelationId.eq(payload.activity.id.to_string()))
        .one(db)
        .await?;
    if let Some(activity) = existing {
        update_existing_activity(db, uploads_dir, activity, import_payload, &profile).await
    } else {
        let result = persist_activity_upload_with_artifacts(
            db,
            PersistActivityUploadWithArtifactsRequest {
                uploads_dir,
                user_storage_key: &user.pid.to_string(),
                user_id,
                upload: import_payload,
                primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_PROVIDER_PAYLOAD,
                primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_STRAVA_STREAMS,
                additional_artifacts: Vec::new(),
                source: "strava_sync",
                deduplication: ActivityUploadDeduplication::Enabled,
                training_profile: Some(&profile),
            },
        )
        .await?;
        if let PersistActivityUploadOutcome::Imported(imported) = result {
            return Ok(Rebuild::import(
                imported.import.id,
                imported.affected_segment_ids,
                imported.fitness_dirty_from_day,
            ));
        }
        Ok(Rebuild::default())
    }
}

async fn update_existing_activity(
    db: &DatabaseConnection,
    uploads_dir: &str,
    activity: activities::Model,
    import_payload: ActivityUploadPayload,
    profile: &crate::training_profile::TrainingProfile,
) -> Result<Rebuild, AppError> {
    let import_id = activity
        .activity_import_id
        .ok_or_else(|| AppError::internal("Strava activity has no import record"))?;
    let activity_import = activity_imports::Entity::find_owned(db, activity.user_id, import_id)
        .await?
        .ok_or_else(|| AppError::internal("Strava import record was not found"))?;
    let activity_import =
        replace_stored_artifacts(db, uploads_dir, &activity_import, &import_payload).await?;
    let prior_day = activity.started_at.date_naive();
    let result = reprocess_activity_from_import(
        db,
        uploads_dir,
        activity.user_id,
        activity.clone(),
        activity_import,
        Some(profile),
    )
    .await?;
    Ok(Rebuild::import(
        import_id,
        result.affected_segment_ids,
        prior_day.min(result.fitness_dirty_from_day),
    ))
}

async fn replace_stored_artifacts(
    db: &DatabaseConnection,
    uploads_dir: &str,
    activity_import: &activity_imports::Model,
    payload: &ActivityUploadPayload,
) -> Result<activity_imports::Model, AppError> {
    let object = crate::strava::objects::ObjectStore::new(uploads_dir)
        .write(
            &super::paths::sources(activity_import.user_id),
            &payload.bytes,
        )
        .await?;
    let transaction = db.begin().await?;
    let artifact = activity_import_artifacts::Entity::store_strava_payload(
        &transaction,
        activity_import,
        object.path,
        payload,
    )
    .await?;
    if !activity_import
        .promote_source(&transaction, &artifact)
        .await?
    {
        return Err(AppError::bad_request(
            "Strava import changed during payload replacement",
        ));
    }
    transaction.commit().await?;
    activity_imports::Entity::find_owned(db, activity_import.user_id, activity_import.id)
        .await?
        .ok_or_else(|| AppError::not_found("Strava import record was removed"))
}
