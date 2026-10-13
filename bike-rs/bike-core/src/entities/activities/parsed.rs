//! Shared persistence of normalized activity data.
use super::*;
use crate::{
    activity_details::serialize_derived_activity_data,
    activity_parser::ParsedActivityData,
    activity_type::ActivityType,
    training_profile::{
        serialize_activity_heart_rate_zones, summarize_heart_rate_zones, TrainingProfile,
    },
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
        parsed
            .draft
            .moving_time_seconds
            .or(parsed.draft.total_time_seconds),
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
            build_save_data(parsed, &import.original_filename, profile)?,
        );
        if update {
            active.update(db).await.map_err(WorkflowError::from)
        } else {
            active.insert(db).await.map_err(WorkflowError::from)
        }
    }
}
