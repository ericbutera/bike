//! Shared persistence of normalized activity data.

#[cfg(test)]
mod activity_reprocessing_tests;

use super::*;
use crate::activity_import_pipeline::{ActivityProcessingArtifact, ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL};
use crate::entities::{activities, activity_imports};
use crate::workflow_error::WorkflowError as AppError;
use crate::{
    activity_details::serialize_derived_activity_data,
    activity_parser::ParsedActivityData,
    activity_type::ActivityType,
    training_profile::{serialize_activity_heart_rate_zones, summarize_heart_rate_zones, TrainingProfile},
    workflow_error::WorkflowError,
};
pub(crate) struct ActivitySaveData {
    pub activity_type: ActivityType,
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zones_json: Option<crate::training_profile::StoredActivityHeartRateZones>,
    pub derived_data_json: crate::activity_details::StoredActivityDerivedData,
}

pub(crate) fn build_save_data(
    parsed: &ParsedActivityData,
    filename: &str,
    profile: &TrainingProfile,
) -> Result<ActivitySaveData, WorkflowError> {
    let zones = summarize_heart_rate_zones(
        &parsed.derived_data.route_points,
        &parsed.derived_data.chart_points,
        parsed.draft.moving_time_seconds.or(parsed.draft.total_time_seconds),
        parsed.draft.average_heart_rate_bpm,
        profile.heart_rate_zone_bounds_bpm.as_deref(),
    );
    Ok(ActivitySaveData {
        activity_type: ActivityType::infer(&parsed.draft.title, filename),
        estimated_ftp_watts: profile.estimated_ftp_watts,
        heart_rate_zones_json: serialize_activity_heart_rate_zones(&zones)?,
        derived_data_json: serialize_derived_activity_data(&parsed.derived_data)?,
    })
}

pub(crate) fn apply_fields(
    active: &mut ActiveModel,
    filename: &str,
    format: &str,
    parsed: &ParsedActivityData,
    data: ActivitySaveData,
) {
    active.title = Set(parsed.draft.title.clone());
    active.sport = Set(parsed.draft.sport.clone());
    active.original_filename = Set(Some(filename.into()));
    active.format = Set(Some(format.into()));
    active.activity_type = Set(data.activity_type.as_str().into());
    active.started_at = Set(parsed.draft.started_at);
    active.ended_at = Set(parsed.draft.ended_at);
    active.distance_meters = Set(parsed.draft.distance_meters);
    active.moving_time_seconds = Set(parsed.draft.moving_time_seconds);
    active.total_time_seconds = Set(parsed.draft.total_time_seconds);
    active.elevation_gain_meters = Set(parsed.draft.elevation_gain_meters);
    active.elevation_loss_meters = Set(parsed.draft.elevation_loss_meters);
    active.average_speed_mps = Set(parsed.draft.average_speed_mps);
    active.max_speed_mps = Set(parsed.draft.max_speed_mps);
    active.average_heart_rate_bpm = Set(parsed.draft.average_heart_rate_bpm);
    active.max_heart_rate_bpm = Set(parsed.draft.max_heart_rate_bpm);
    active.average_cadence_rpm = Set(parsed.draft.average_cadence_rpm);
    active.max_cadence_rpm = Set(parsed.draft.max_cadence_rpm);
    active.calories = Set(parsed.draft.calories);
    active.estimated_ftp_watts = Set(data.estimated_ftp_watts);
    active.heart_rate_zones_json = Set(data.heart_rate_zones_json);
    active.derived_data_json = Set(Some(data.derived_data_json));
}

impl Model {
    pub async fn save_parsed_strava(
        db: &impl ConnectionTrait,
        event: &crate::strava::events::StravaEvent,
        import: &crate::entities::activity_imports::Model,
        parsed: &ParsedActivityData,
        profile: &TrainingProfile,
        summary: &crate::strava::provider_payload::StravaActivitySummary,
    ) -> Result<Self, WorkflowError> {
        let existing = Self::strava_activity(db, event.bike_user_id, event.activity_id()?).await?;
        let update = existing.is_some();
        let mut active = existing.map(ActiveModel::from).unwrap_or_default();
        active.user_id = Set(event.bike_user_id);
        active.activity_import_id = Set(Some(import.id));
        active.source = Set("strava_sync".into());
        active.source_correlation_id = Set(event.strava_activity_id.map(|id| id.to_string()));
        apply_fields(
            &mut active,
            &import.original_filename,
            "json",
            parsed,
            build_strava_save_data(parsed, &import.original_filename, profile, summary)?,
        );
        if update {
            active.update(db).await.map_err(WorkflowError::from)
        } else {
            active.insert(db).await.map_err(WorkflowError::from)
        }
    }
}

fn build_strava_save_data(
    parsed: &ParsedActivityData,
    filename: &str,
    profile: &TrainingProfile,
    summary: &crate::strava::provider_payload::StravaActivitySummary,
) -> Result<ActivitySaveData, WorkflowError> {
    let mut data = build_save_data(parsed, filename, profile)?;
    data.derived_data_json = data.derived_data_json.with_source_metadata(summary.source_metadata());
    if !crate::activity_sport::is_bike_activity_sport(&parsed.draft.sport) {
        data.estimated_ftp_watts = None;
        data.heart_rate_zones_json = None;
    }
    Ok(data)
}

impl Model {
    pub(crate) async fn update_parsed(
        self,
        db: &impl ConnectionTrait,
        parsing_artifact: &ActivityProcessingArtifact,
        parsed: &ParsedActivityData,
        save_data: ActivitySaveData,
    ) -> Result<activities::Model, AppError> {
        let expected_updated_at = self.updated_at;
        let activity_type = self.activity_type.clone();
        let archive_title = (self.source == "strava_sync"
            && parsing_artifact.artifact_kind == ACTIVITY_IMPORT_ARTIFACT_KIND_ORIGINAL)
            .then(|| self.title.clone());
        let mut active_model: activities::ActiveModel = self.into();
        apply_fields(
            &mut active_model,
            &parsing_artifact.original_filename,
            &parsing_artifact.format,
            parsed,
            save_data,
        );
        active_model.activity_type = Set(activity_type);
        if let Some(title) = archive_title {
            active_model.title = Set(title);
        }
        let active_model = active_model.before_save(db, false).await?;
        activities::Entity::update(active_model)
            .validate()?
            .filter(activities::Column::UpdatedAt.eq(expected_updated_at))
            .exec(db)
            .await
            .map_err(AppError::from)
    }

    pub(crate) async fn insert_parsed(
        db: &DatabaseConnection,
        import: &activity_imports::Model,
        parsing_artifact: &ActivityProcessingArtifact,
        parsed: &ParsedActivityData,
        save_data: ActivitySaveData,
        source_correlation_id: Option<String>,
    ) -> Result<activities::Model, AppError> {
        let mut active_model = activities::ActiveModel {
            user_id: Set(import.user_id),
            activity_import_id: Set(Some(import.id)),
            source: Set(import.source.clone()),
            source_correlation_id: Set(source_correlation_id),
            ..Default::default()
        };
        apply_fields(
            &mut active_model,
            &parsing_artifact.original_filename,
            &parsing_artifact.format,
            parsed,
            save_data,
        );
        active_model.insert(db).await.map_err(AppError::from)
    }
}

impl Model {
    pub async fn adopt_source_metadata(
        self,
        db: &impl ConnectionTrait,
        artifact: &crate::entities::activity_import_artifacts::Model,
    ) -> Result<Self, DbErr> {
        let mut activity: ActiveModel = self.into();
        activity.original_filename = Set(Some(artifact.original_filename.clone()));
        activity.format = Set(Some(artifact.format.clone()));
        activity.update(db).await
    }
}

#[cfg(test)]
mod strava_native_unit {
    use super::*;

    #[test]
    fn summary_storage_preserves_gear_and_description_and_excludes_non_cycling_training() {
        use crate::strava::provider_payload::{parse_strava_provider_payload, StoredStravaProviderPayload};
        let mut payload: StoredStravaProviderPayload =
            serde_json::from_slice(crate::strava::fixtures::REAL_RIDE).unwrap();
        payload
            .activity
            .additional_fields
            .insert("description".into(), serde_json::json!("Fixture description"));
        payload.activity.additional_fields.insert(
            "gear".into(),
            serde_json::json!({"id":"fixture-bike","name":"Fixture bicycle"}),
        );
        for sport in ["Ride", "Walk", "Swim", "Workout"] {
            payload.activity.sport_type = Some(sport.into());
            let parsed = parse_strava_provider_payload(&serde_json::to_vec(&payload).unwrap()).unwrap();
            let profile = TrainingProfile {
                estimated_ftp_watts: Some(250),
                ..Default::default()
            };
            let saved = build_strava_save_data(&parsed, "fixture.json", &profile, &payload.activity).unwrap();
            let json = serde_json::to_vec(&saved.derived_data_json).unwrap();
            let restored: crate::activity_data::StoredActivityDerivedData = serde_json::from_slice(&json).unwrap();
            assert_eq!(restored.source_metadata(), Some(&payload.activity.source_metadata()));
            if sport != "Ride" {
                assert!(saved.estimated_ftp_watts.is_none());
                assert!(saved.heart_rate_zones_json.is_none());
            }
        }
    }
}
