use crate::{
    activity_parser::{parse_activity_data, ParsedActivityData},
    bike_tasks::{ImportExecution, ImportTarget},
    entities::{activities, activity_import_artifacts, activity_imports, activity_training_analyses},
};

pub const REPROCESS_SOURCE: &[u8] = super::recordings::OUTDOOR_TCX;
pub const REPROCESS_RECORDING_SOURCE: &[u8] = super::recordings::VIRTUAL_GENERIC_FIT;

pub fn reprocess_activity() -> activities::Model {
    activities::Model {
        activity_import_id: Some(super::activity_import().id),
        ..super::activity()
    }
}

pub fn reprocess_import() -> activity_imports::Model {
    activity_imports::Model {
        activity_id: Some(super::activity().id),
        format: "tcx".into(),
        original_filename: "outdoor.tcx".into(),
        size_bytes: REPROCESS_SOURCE.len() as i64,
        ..super::activity_import()
    }
}

pub fn reprocess_artifact() -> activity_import_artifacts::Model {
    let import = reprocess_import();
    activity_import_artifacts::Model {
        format: import.format,
        source_quality: "tcx_original".into(),
        original_filename: import.original_filename,
        size_bytes: import.size_bytes,
        checksum_sha256: crate::object_store::checksum(REPROCESS_SOURCE),
        ..super::import_artifact()
    }
}

pub fn reprocess_recording_artifact() -> activity_import_artifacts::Model {
    let source = reprocess_artifact();
    activity_import_artifacts::Model {
        id: source.id + 1,
        format: "fit".into(),
        source_quality: "fit_original".into(),
        original_filename: "virtual-generic.fit".into(),
        storage_path: "activity-imports/fixture-recording.fit".into(),
        size_bytes: REPROCESS_RECORDING_SOURCE.len() as i64,
        checksum_sha256: crate::object_store::checksum(REPROCESS_RECORDING_SOURCE),
        ..source
    }
}

pub fn reprocess_execution() -> ImportExecution {
    let activity = reprocess_activity();
    let artifact = reprocess_artifact();
    let mut execution = super::import_execution();
    execution.command.source.object = crate::object_store::ObjectRef {
        path: artifact.storage_path,
        sha256: artifact.checksum_sha256,
        bytes: REPROCESS_SOURCE.len(),
    };
    execution.command.target = ImportTarget::ExistingActivity {
        activity_id: activity.id,
    };
    execution.workflow.workflow_type = "reprocess_activity_import".into();
    execution.workflow.job_id = format!(
        "bike-task-reprocess_activity_import-user-{}-{}",
        activity.user_id, execution.command.source.command_id,
    );
    execution
}

pub fn parsed_reprocess_activity() -> ParsedActivityData {
    parse_activity_data("outdoor.tcx", "tcx", REPROCESS_SOURCE).expect("valid recording fixture")
}

pub fn training_analysis() -> activity_training_analyses::Model {
    activity_training_analyses::Model {
        activity_id: super::activity().id,
        user_id: super::activity().user_id,
        ride_focus: "other".into(),
        route_family_key: None,
        comparable_distance_bucket_meters: None,
        comparable_elevation_gain_bucket_meters: None,
        aerobic_decoupling_percent: None,
        z2_time_seconds: 0,
        z2_distance_meters: None,
        z2_average_speed_mps: None,
        climbing_time_seconds: 0,
        climbing_elevation_gain_meters: None,
        sustained_climb_count: 0,
        created_at: super::timestamp(),
        updated_at: super::timestamp(),
    }
}
