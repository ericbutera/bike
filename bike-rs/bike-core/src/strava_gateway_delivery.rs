use crate::activity_import_pipeline::{
    finalize_activity_import_batch, mark_activity_imports_processed,
    persist_activity_upload_with_artifacts, reprocess_activity_from_import,
    ActivityImportArtifactPayload, ActivityUploadDeduplication, PersistActivityUploadOutcome,
    PersistActivityUploadWithArtifactsRequest, ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
    ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
};
use crate::auth::entities::users;
use crate::entities::{
    activities, activity_import_artifacts, activity_imports, strava_connections, strava_gateway,
};
use crate::jobs::JobQueue as TaskQueue;
use crate::strava::{
    build_activity_upload, delete_strava_activity, delete_strava_activity_by_correlation_id,
    forget_local_connection,
};
use crate::strava_provider_payload::{
    strava_activity_is_bike, StravaActivityStreams, StravaActivitySummary,
};
use crate::training_profile::load_training_profile;
use crate::workflow_error::WorkflowError as AppError;
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct GatewayPayload {
    pub activity: StravaActivitySummary,
    #[serde(default)]
    pub streams: StravaActivityStreams,
}

pub async fn receive(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    receipt: &strava_gateway::Receipt<'_>,
    payload: Option<GatewayPayload>,
) -> Result<strava_gateway::Claim, AppError> {
    validate_existing_connection(db, receipt).await?;
    let claim = strava_gateway::claim(db, receipt).await?;
    if claim != strava_gateway::Claim::Acquired {
        return Ok(claim);
    }

    let result = apply(db, tasks, uploads_dir, receipt, payload).await;
    if let Err(error) = result {
        strava_gateway::release(db, receipt, &error).await?;
        return Err(error);
    }
    strava_gateway::complete(db, receipt).await?;
    Ok(strava_gateway::Claim::Acquired)
}

async fn validate_existing_connection(
    db: &DatabaseConnection,
    receipt: &strava_gateway::Receipt<'_>,
) -> Result<(), AppError> {
    if let Some(connection) = strava_connections::Entity::find()
        .filter(strava_connections::Column::UserId.eq(receipt.user_id))
        .one(db)
        .await?
    {
        if connection.athlete_id != receipt.athlete_id {
            return Err(AppError::conflict(
                "Rust Strava connection belongs to another athlete",
            ));
        }
    }
    Ok(())
}

async fn apply(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    receipt: &strava_gateway::Receipt<'_>,
    payload: Option<GatewayPayload>,
) -> Result<(), AppError> {
    match receipt.operation {
        "upsert" => {
            let payload =
                payload.ok_or_else(|| AppError::bad_request("Missing Strava activity"))?;
            if payload.activity.id != receipt.activity_id {
                return Err(AppError::bad_request(
                    "Strava activity ID does not match delivery",
                ));
            }
            if strava_activity_is_bike(&payload.activity) {
                upsert_activity(db, tasks, uploads_dir, receipt.user_id, &payload).await
            } else {
                delete_activity(db, tasks, uploads_dir, receipt.user_id, receipt.activity_id).await
            }
        }
        "delete" => {
            delete_activity(db, tasks, uploads_dir, receipt.user_id, receipt.activity_id).await
        }
        "deauthorize" => deauthorize(db, tasks, uploads_dir, receipt.user_id).await,
        _ => Err(AppError::bad_request("Invalid Strava gateway operation")),
    }
}

async fn delete_activity(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    user_id: i32,
    activity_id: i64,
) -> Result<(), AppError> {
    delete_strava_activity_by_correlation_id(db, uploads_dir, tasks, user_id, activity_id).await?;
    Ok(())
}

async fn deauthorize(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    user_id: i32,
) -> Result<(), AppError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Source.eq("strava_sync"))
        .all(db)
        .await?;
    for activity in activities {
        delete_strava_activity(db, uploads_dir, tasks, user_id, activity).await?;
    }
    forget_local_connection(db, user_id).await
}

async fn upsert_activity(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    user_id: i32,
    payload: &GatewayPayload,
) -> Result<(), AppError> {
    let user = users::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::not_found("Bike user for Strava delivery was not found"))?;
    let profile = load_training_profile(db, user_id).await?;
    let import_payload = build_activity_upload(&payload.activity, &payload.streams)?;
    let existing = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Source.eq("strava_sync"))
        .filter(activities::Column::SourceCorrelationId.eq(payload.activity.id.to_string()))
        .one(db)
        .await?;
    if let Some(activity) = existing {
        update_existing_activity(db, tasks, uploads_dir, activity, import_payload, &profile).await
    } else {
        let result = persist_activity_upload_with_artifacts(
            db,
            PersistActivityUploadWithArtifactsRequest {
                uploads_dir,
                user_storage_key: &user.pid.to_string(),
                user_id,
                upload: import_payload.generated_tcx_upload,
                primary_artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT,
                primary_source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX,
                additional_artifacts: vec![import_payload.provider_payload_artifact],
                source: "strava_sync",
                deduplication: ActivityUploadDeduplication::Enabled,
                training_profile: Some(&profile),
            },
        )
        .await?;
        if let PersistActivityUploadOutcome::Imported(imported) = result {
            finalize_activity_import_batch(
                db,
                tasks,
                user_id,
                imported.affected_segment_ids,
                Some(imported.fitness_dirty_from_day),
                Utc::now(),
            )
            .await?;
            mark_activity_imports_processed(db, &[imported.import.id]).await?;
        }
        Ok(())
    }
}

async fn update_existing_activity(
    db: &DatabaseConnection,
    tasks: &TaskQueue,
    uploads_dir: &str,
    activity: activities::Model,
    import_payload: crate::strava::StravaActivityImportPayload,
    profile: &crate::training_profile::TrainingProfile,
) -> Result<(), AppError> {
    let import_id = activity
        .activity_import_id
        .ok_or_else(|| AppError::internal("Strava activity has no import record"))?;
    let activity_import = activity_imports::Entity::find_by_id(import_id)
        .one(db)
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
    finalize_activity_import_batch(
        db,
        tasks,
        activity.user_id,
        result.affected_segment_ids,
        Some(prior_day.min(result.fitness_dirty_from_day)),
        Utc::now(),
    )
    .await?;
    mark_activity_imports_processed(db, &[import_id]).await?;
    Ok(())
}

async fn replace_stored_artifacts(
    db: &DatabaseConnection,
    uploads_dir: &str,
    activity_import: &activity_imports::Model,
    payload: &crate::strava::StravaActivityImportPayload,
) -> Result<activity_imports::Model, AppError> {
    let parent = Path::new(&activity_import.storage_path)
        .parent()
        .ok_or_else(|| AppError::internal("Strava import path has no parent"))?;
    store_updated_artifact(
        db,
        uploads_dir,
        parent,
        activity_import,
        &payload.provider_payload_artifact,
    )
    .await?;
    let export = &payload.generated_tcx_upload;
    let export_artifact = ActivityImportArtifactPayload {
        artifact_kind: ACTIVITY_IMPORT_ARTIFACT_KIND_GENERATED_EXPORT.to_string(),
        format: export.format.clone(),
        source_quality: ACTIVITY_IMPORT_SOURCE_QUALITY_GENERATED_TCX.to_string(),
        original_filename: export.original_filename.clone(),
        mime_type: export.mime_type.clone(),
        bytes: export.bytes.clone(),
    };
    let export_path =
        store_updated_artifact(db, uploads_dir, parent, activity_import, &export_artifact).await?;
    let mut model: activity_imports::ActiveModel = activity_import.clone().into();
    model.storage_path = Set(export_path);
    model.original_filename = Set(export.original_filename.clone());
    model.size_bytes = Set(export.bytes.len() as i64);
    model.update(db).await.map_err(AppError::from)
}

async fn store_updated_artifact(
    db: &DatabaseConnection,
    uploads_dir: &str,
    parent: &Path,
    activity_import: &activity_imports::Model,
    artifact: &ActivityImportArtifactPayload,
) -> Result<String, AppError> {
    let relative_path = parent.join(format!(
        "strava_gateway_{}.{}",
        Uuid::new_v4(),
        artifact.format
    ));
    let full_path = Path::new(uploads_dir).join(&relative_path);
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&full_path, &artifact.bytes).await?;
    let relative_path = relative_path.to_string_lossy().to_string();
    let checksum = hex::encode(Sha256::digest(&artifact.bytes));
    let existing = activity_import_artifacts::Entity::find()
        .filter(activity_import_artifacts::Column::ActivityImportId.eq(activity_import.id))
        .filter(activity_import_artifacts::Column::ArtifactKind.eq(&artifact.artifact_kind))
        .one(db)
        .await?;
    if let Some(existing) = existing {
        let mut model: activity_import_artifacts::ActiveModel = existing.into();
        model.storage_path = Set(relative_path.clone());
        model.original_filename = Set(artifact.original_filename.clone());
        model.size_bytes = Set(artifact.bytes.len() as i64);
        model.checksum_sha256 = Set(checksum);
        model.update(db).await?;
    } else {
        activity_import_artifacts::ActiveModel {
            activity_import_id: Set(activity_import.id),
            user_id: Set(activity_import.user_id),
            artifact_kind: Set(artifact.artifact_kind.clone()),
            format: Set(artifact.format.clone()),
            source_quality: Set(artifact.source_quality.clone()),
            original_filename: Set(artifact.original_filename.clone()),
            storage_path: Set(relative_path.clone()),
            size_bytes: Set(artifact.bytes.len() as i64),
            mime_type: Set(artifact.mime_type.clone()),
            checksum_sha256: Set(checksum),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }
    Ok(relative_path)
}

#[cfg(test)]
mod tests {
    use super::{receive, GatewayPayload};
    use crate::auth::entities::users;
    use crate::entities::{
        activities, integration_events, strava_gateway::Receipt,
        strava_gateway_bindings as bindings, strava_gateway_receipts as receipts,
        strava_gateway_revocations as revocations, strava_gateway_watermarks as watermarks,
    };
    use crate::jobs::JobQueue as TaskQueue;
    use sea_orm::{
        ActiveModelTrait, ColumnTrait, Database, DatabaseConnection, EntityTrait, QueryFilter, Set,
    };
    use serde_json::json;
    use uuid::Uuid;

    #[tokio::test]
    async fn imports_original_ride_fixture_without_duplicate() {
        let Ok(url) = std::env::var("BIKE_TEST_DATABASE_URL") else {
            return;
        };
        let db = Database::connect(&url).await.unwrap();
        let (unique, user_id, athlete_id, uploads_dir) = prepare_fixture(&db).await;
        let tasks = TaskQueue::new(db.clone());
        let delivery_id = format!("rust:{unique}:fixture");
        let receipt = Receipt {
            delivery_id: &delivery_id,
            athlete_id,
            user_id,
            activity_id: 887654322,
            event_time: 100,
            operation: "upsert",
        };
        let payload =
            || serde_json::from_slice(include_bytes!("../testdata/strava-real-ride.json")).unwrap();
        receive(&db, &tasks, &uploads_dir, &receipt, Some(payload()))
            .await
            .unwrap();
        receive(&db, &tasks, &uploads_dir, &receipt, Some(payload()))
            .await
            .unwrap();
        let imported = activities::Entity::find()
            .filter(activities::Column::UserId.eq(user_id))
            .filter(activities::Column::SourceCorrelationId.eq("887654322"))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].title, "Morning Ride fixture");
        assert_eq!(imported[0].source, "strava_sync");
        assert_eq!(imported[0].distance_meters, Some(67.637657));
        let data = crate::activity_data::deserialize_derived_activity_data(
            imported[0].derived_data_json.as_ref(),
        );
        assert_eq!(data.route_points.len(), 8);
        assert_eq!(data.route_points[0].latitude, 44.742805);
        assert_applied_events(&db, user_id, 1).await;
        cleanup_fixture(&db, athlete_id, user_id, &uploads_dir).await;
    }

    #[tokio::test]
    async fn failed_delivery_records_its_error_and_recovers_on_retry() {
        let Ok(url) = std::env::var("BIKE_TEST_DATABASE_URL") else {
            return;
        };
        let db = Database::connect(&url).await.unwrap();
        let (unique, user_id, athlete_id, uploads_dir) = prepare_fixture(&db).await;
        let tasks = TaskQueue::new(db.clone());
        let delivery_id = format!("rust:{unique}:retry");
        let receipt = Receipt {
            delivery_id: &delivery_id,
            athlete_id,
            user_id,
            activity_id: 42,
            event_time: 100,
            operation: "upsert",
        };
        let error = receive(&db, &tasks, &uploads_dir, &receipt, None)
            .await
            .unwrap_err();
        assert_eq!(error.message, "Missing Strava activity");
        receive(
            &db,
            &tasks,
            &uploads_dir,
            &receipt,
            Some(ride_payload("Recovered Ride", 1000.0, 100, 44.001)),
        )
        .await
        .unwrap();
        let events = crate::integration_events_service::list_recent_events(
            &db,
            crate::integration_events_service::IntegrationEventListOptions {
                provider: Some("strava".into()),
                user_id: Some(user_id),
                activity_id: None,
                import_id: None,
                limit: 100,
            },
        )
        .await
        .unwrap();
        assert_eq!(events.len(), 4);
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "gateway.delivery.received")
                .count(),
            2
        );
        let failure = events
            .iter()
            .find(|event| event.event_type == "gateway.delivery.failed")
            .unwrap();
        assert_eq!(failure.message, "Missing Strava activity");
        assert_eq!(failure.level, "error");
        assert_eq!(failure.payload.as_ref().unwrap()["status_code"], 400);
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "gateway.delivery.applied")
                .count(),
            1
        );
        cleanup_fixture(&db, athlete_id, user_id, &uploads_dir).await;
    }

    #[tokio::test]
    async fn imported_ride_updates_in_place_and_delete_removes_it() {
        let Ok(url) = std::env::var("BIKE_TEST_DATABASE_URL") else {
            return;
        };
        let db = Database::connect(&url)
            .await
            .expect("connect to migrated test database");
        let (unique, user_id, athlete_id, uploads_dir) = prepare_fixture(&db).await;
        let uploads_dir = uploads_dir.as_str();
        let tasks = TaskQueue::new(db.clone());
        let first_id = format!("rust:{unique}:1");
        let first = Receipt {
            delivery_id: &first_id,
            athlete_id,
            user_id,
            activity_id: 42,
            event_time: 100,
            operation: "upsert",
        };
        receive(
            &db,
            &tasks,
            uploads_dir,
            &first,
            Some(ride_payload("First Ride", 1000.0, 100, 44.001)),
        )
        .await
        .unwrap();
        let imported = activities::Entity::find()
            .filter(activities::Column::UserId.eq(user_id))
            .filter(activities::Column::SourceCorrelationId.eq("42"))
            .one(&db)
            .await
            .unwrap()
            .expect("ride imported");
        assert_eq!(imported.title, "First Ride");
        let original_id = imported.id;
        let mut model: activities::ActiveModel = imported.into();
        model.activity_type = Set("race".to_string());
        model.update(&db).await.unwrap();

        let second_id = format!("rust:{unique}:2");
        let second = Receipt {
            delivery_id: &second_id,
            event_time: 200,
            ..first
        };
        receive(
            &db,
            &tasks,
            uploads_dir,
            &second,
            Some(ride_payload("Updated Ride", 1500.0, 150, 44.002)),
        )
        .await
        .unwrap();
        let updated = activities::Entity::find_by_id(original_id)
            .one(&db)
            .await
            .unwrap()
            .expect("ride updated in place");
        assert_eq!(updated.title, "Updated Ride");
        assert_eq!(updated.activity_type, "race");
        assert_eq!(updated.distance_meters, Some(1500.0));

        let delete_id = format!("rust:{unique}:3");
        let deletion = Receipt {
            delivery_id: &delete_id,
            event_time: 300,
            operation: "delete",
            ..first
        };
        receive(&db, &tasks, uploads_dir, &deletion, None)
            .await
            .unwrap();
        assert!(activities::Entity::find_by_id(original_id)
            .one(&db)
            .await
            .unwrap()
            .is_none());

        assert_applied_events(&db, user_id, 3).await;

        cleanup_fixture(&db, athlete_id, user_id, uploads_dir).await;
    }

    async fn prepare_fixture(db: &DatabaseConnection) -> (String, i32, i64, String) {
        let unique = Uuid::new_v4().to_string();
        let user_id = users::ActiveModel {
            pid: Set(Uuid::new_v4()),
            email: Set(format!("gateway-delivery-{unique}@example.invalid")),
            api_key: Set(unique.clone()),
            name: Set("Gateway test".into()),
            ..Default::default()
        }
        .insert(db)
        .await
        .expect("create user")
        .id;
        let athlete_id = i64::from(user_id) + 8_000_000_000;
        let uploads_dir = std::env::temp_dir().join(format!("bike-gateway-test-{unique}"));
        tokio::fs::create_dir_all(&uploads_dir).await.unwrap();
        (
            unique,
            user_id,
            athlete_id,
            uploads_dir.to_str().unwrap().to_owned(),
        )
    }

    async fn cleanup_fixture(
        db: &DatabaseConnection,
        athlete_id: i64,
        user_id: i32,
        uploads_dir: &str,
    ) {
        integration_events::Entity::delete_many()
            .filter(integration_events::Column::UserId.eq(user_id))
            .exec(db)
            .await
            .unwrap();
        receipts::Entity::delete_many()
            .filter(receipts::Column::AthleteId.eq(athlete_id))
            .exec(db)
            .await
            .unwrap();
        watermarks::Entity::delete_many()
            .filter(watermarks::Column::AthleteId.eq(athlete_id))
            .exec(db)
            .await
            .unwrap();
        revocations::Entity::delete_by_id(athlete_id)
            .exec(db)
            .await
            .unwrap();
        bindings::Entity::delete_by_id(athlete_id)
            .exec(db)
            .await
            .unwrap();
        users::Entity::delete_by_id(user_id).exec(db).await.unwrap();
        tokio::fs::remove_dir_all(uploads_dir).await.unwrap();
    }

    async fn assert_applied_events(db: &DatabaseConnection, user_id: i32, expected: usize) {
        let events = crate::integration_events_service::list_recent_events(
            db,
            crate::integration_events_service::IntegrationEventListOptions {
                provider: Some("strava".into()),
                user_id: Some(user_id),
                activity_id: None,
                import_id: None,
                limit: 100,
            },
        )
        .await
        .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "gateway.delivery.applied")
                .count(),
            expected
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event.event_type == "gateway.delivery.received")
                .count(),
            expected
        );
    }

    fn ride_payload(name: &str, distance: f64, seconds: i32, end_latitude: f64) -> GatewayPayload {
        serde_json::from_value(json!({
            "activity": {
                "id": 42, "name": name, "sport_type": "Ride",
                "start_date": "2026-09-20T12:00:00Z", "distance": distance,
                "moving_time": seconds, "elapsed_time": seconds + 20
            },
            "streams": {
                "time": {"data": [0, seconds]},
                "distance": {"data": [0.0, distance]},
                "latlng": {"data": [[44.0, -85.0], [end_latitude, -85.001]]}
            }
        }))
        .unwrap()
    }
}
