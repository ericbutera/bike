//! Deterministic model fixtures shared by Rust unit and adapter tests.
//!
//! Override scenario fields with struct update syntax. Use `new_activity_import`
//! for database inserts so unrelated seeds never reuse the fixture primary key.
mod accounts;
pub mod recordings;
mod strava;
pub use strava::{strava_activity_state, strava_event_record};
#[path = "strava/fixture.rs"]
pub mod strava_fixture;
pub const STRAVA_REAL_RIDE: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/strava-real-ride.json"));
mod activity_history;
pub use activity_history::{activity_history, activity_history_event};
mod storage;
pub use storage::object_storage;
mod tasks;
pub mod temporal_client;
pub use tasks::{
    activity_task, fitness_change, import_command, import_execution, import_result, job_reference, reprocess_input,
    segment_change, task_cause,
};
mod analytics;
mod defaults;
pub use analytics::{
    activity_analytics, activity_id_row, analytics_user_state, fitness_day, new_fitness_day, new_segment_effort,
    segment_effort, segment_id_rows,
};
pub use defaults::user_preferences;
pub use defaults::{new_feature_flag, new_user_preferences};
mod heatmaps;
pub use heatmaps::heatmap_projection;
mod imports;
mod integration_events;
pub use integration_events::{integration_event, quota_bucket};
mod mocks;
pub use mocks::{count_row, postgres_mock, written};
mod reprocessing;
pub use reprocessing::{
    parsed_reprocess_activity, reprocess_activity, reprocess_artifact, reprocess_execution, reprocess_import,
    reprocess_recording_artifact, training_analysis, REPROCESS_RECORDING_SOURCE, REPROCESS_SOURCE,
};
mod persistence;
pub use persistence::{insert, update};
mod segments;
pub mod trace;
pub use segments::{personal_achievement, segment, segment_summary, segment_user_summary};

pub use accounts::{auth_event, connection, customer, new_connection, new_customer, refresh_token};
pub use imports::{activity_upload, archive_import_job, artifact_payload, import_artifact};

use crate::entities::{activities, activity_imports};
use chrono::{DateTime, Utc};
use sea_orm::ActiveValue::NotSet;

pub fn timestamp() -> DateTime<Utc> {
    DateTime::from_timestamp(1788220800, 0).expect("fixed fixture timestamp")
}

pub fn activity_import() -> activity_imports::Model {
    let at = timestamp();
    activity_imports::Model {
        id: 18,
        user_id: 7,
        import_version: activity_imports::ACTIVITY_IMPORT_VERSION_CURRENT,
        source: "manual_upload".into(),
        format: "gpx".into(),
        status: "uploaded".into(),
        activity_id: None,
        archive_job_id: None,
        processing_stage: "raw_stored".into(),
        processing_error: None,
        processing_attempts: 0,
        processed_at: None,
        last_processing_event_at: Some(at),
        original_filename: "fixture.gpx".into(),
        storage_path: "activity-imports/fixture.gpx".into(),
        size_bytes: 123,
        mime_type: Some("application/gpx+xml".into()),
        created_at: at,
        updated_at: at,
    }
}

pub fn new_activity_import() -> activity_imports::ActiveModel {
    let mut active: activity_imports::ActiveModel = activity_import().into();
    active.id = NotSet;
    active
}

pub fn activity() -> activities::Model {
    let at = timestamp();
    activities::Model {
        id: 42,
        user_id: 7,
        activity_import_id: None,
        title: "Fixture ride".into(),
        sport: "ride".into(),
        source: "manual_upload".into(),
        source_correlation_id: None,
        original_filename: None,
        format: None,
        activity_type: "training".into(),
        started_at: at,
        ended_at: None,
        distance_meters: None,
        moving_time_seconds: None,
        total_time_seconds: None,
        elevation_gain_meters: None,
        elevation_loss_meters: None,
        average_speed_mps: None,
        max_speed_mps: None,
        average_heart_rate_bpm: None,
        max_heart_rate_bpm: None,
        average_cadence_rpm: None,
        max_cadence_rpm: None,
        calories: None,
        estimated_ftp_watts: None,
        heart_rate_zones_json: None,
        derived_data_json: None,
        created_at: at,
        updated_at: at,
    }
}

pub fn new_activity() -> activities::ActiveModel {
    let mut active: activities::ActiveModel = activity().into();
    active.id = NotSet;
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_fixtures_share_a_stable_owner_and_clock() {
        let customer = customer();
        let connection = connection();
        let activity = activity();
        let import = activity_import();
        let artifact = import_artifact();
        let archive = archive_import_job();
        let state = analytics_user_state();
        let day = fitness_day();
        let effort = segment_effort();
        assert_eq!(activity.user_id, customer.id);
        assert_eq!(import.user_id, customer.id);
        assert_eq!(connection.user_id, customer.id);
        assert_eq!(artifact.user_id, customer.id);
        assert_eq!(artifact.activity_import_id, import.id);
        assert_eq!(archive.user_id, customer.id);
        assert_eq!(state.user_id, customer.id);
        assert_eq!(day.user_id, customer.id);
        assert_eq!(effort.user_id, customer.id);
        assert_eq!(activity.created_at, customer.created_at);
        assert_eq!(import.created_at, customer.created_at);
        assert_eq!(artifact.created_at, customer.created_at);
        assert_eq!(archive.created_at, customer.created_at);
        assert_eq!(state.created_at, customer.created_at);
        assert_eq!(day.created_at, customer.created_at);
        assert_eq!(effort.created_at, customer.created_at);
        assert_eq!(activity, super::activity());
        assert_eq!(import, activity_import());
    }
}
