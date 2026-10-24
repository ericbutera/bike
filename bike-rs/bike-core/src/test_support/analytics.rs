use crate::entities::{analytics_user_states, fitness_freshness_daily, segment_efforts};
pub fn analytics_user_state() -> analytics_user_states::Model {
    let at = super::timestamp();
    analytics_user_states::Model {
        user_id: 7,
        last_activity_change_at: at,
        fitness_dirty_from_day: None,
        last_fitness_rebuild_at: None,
        created_at: at,
        updated_at: at,
    }
}
pub fn fitness_day() -> fitness_freshness_daily::Model {
    let at = super::timestamp();
    fitness_freshness_daily::Model {
        id: 1,
        user_id: 7,
        day: at.date_naive(),
        activity_count: 1,
        training_load: 10.0,
        fitness: 1.0,
        fatigue: 2.0,
        form: -1.0,
        created_at: at,
        updated_at: at,
    }
}
pub fn segment_effort() -> segment_efforts::Model {
    let at = super::timestamp();
    segment_efforts::Model {
        id: 1,
        user_id: 7,
        segment_id: 1,
        activity_id: 42,
        effort_index: 1,
        start_route_point_index: 0,
        end_route_point_index: 1,
        start_elapsed_seconds: 0,
        end_elapsed_seconds: 60,
        duration_seconds: 60,
        distance_meters: Some(1000.0),
        overall_rank: None,
        user_rank: None,
        created_at: at,
        updated_at: at,
    }
}

pub fn segment_id_rows(ids: &[i32]) -> Vec<std::collections::BTreeMap<String, sea_orm::Value>> {
    ids.iter()
        .map(|id| [("segment_id".into(), (*id).into())].into())
        .collect()
}
pub fn new_fitness_day() -> fitness_freshness_daily::ActiveModel {
    let mut active: fitness_freshness_daily::ActiveModel = fitness_day().into();
    active.id = sea_orm::ActiveValue::NotSet;
    active
}
pub fn new_segment_effort() -> segment_efforts::ActiveModel {
    let mut active: segment_efforts::ActiveModel = segment_effort().into();
    active.id = sea_orm::ActiveValue::NotSet;
    active
}

pub fn activity_analytics() -> crate::entities::activity_analytics::Model {
    let activity = super::activity();
    crate::entities::activity_analytics::Model {
        activity_id: activity.id,
        user_id: activity.user_id,
        segment_effort_count: 1,
        achievement_count: 0,
        kom_count: 0,
        top_10_count: 0,
        pr_count: 0,
        achievement_highlights_json: None,
        created_at: super::timestamp(),
        updated_at: super::timestamp(),
    }
}

pub fn activity_id_row() -> std::collections::BTreeMap<String, sea_orm::Value> {
    [("id".into(), super::activity().id.into())].into()
}

pub fn activity_award_output() -> crate::entities::activity_award_outputs::Model {
    let activity = super::activity();
    crate::entities::activity_award_outputs::Model {
        activity_id: activity.id,
        user_id: activity.user_id,
    }
}

pub fn activity_earned_awards() -> crate::entities::activity_earned_awards::Model {
    let effort = segment_effort();
    crate::entities::activity_earned_awards::Model {
        activity_id: effort.activity_id,
        position: 1,
        display_position: 1,
        segment_id: effort.segment_id,
        segment_title: "Riverfront climb".into(),
        effort_index: effort.effort_index,
        overall_rank: Some(1),
        personal_rank: Some(1),
        personal_best_duration_seconds: Some(effort.duration_seconds),
    }
}

pub fn activity_climb_output() -> crate::entities::activity_climbs::output::Model {
    let activity = super::activity();
    let recording = crate::activity_data::deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    crate::entities::activity_climbs::output::Model {
        activity_id: activity.id,
        detector_version: crate::activity_climbs::DETECTOR_VERSION,
        recording_sha256: crate::object_store::checksum(&serde_json::to_vec(&recording).unwrap()),
        object_path: "activity-imports/derived/fixture/climbs.json".into(),
        object_sha256: crate::object_store::checksum(b"fixture climbs"),
        object_bytes: 14,
        climb_count: 0,
    }
}

pub fn activity_climb() -> crate::entities::activity_climbs::Model {
    crate::entities::activity_climbs::Model {
        activity_id: super::activity().id,
        climb_number: 1,
        start_route_point_index: None,
        end_route_point_index: None,
        start_elapsed_seconds: 0,
        end_elapsed_seconds: 120,
        duration_seconds: 120,
        distance_meters: 520.0,
        elevation_gain_meters: 26.0,
        average_grade_percent: Some(5.0),
        elevation_loss_meters: Some(0.0),
        max_grade_percent: Some(6.0),
        category: None,
        route_points_json: Default::default(),
        vertical_rate_meters_per_hour: Some(780.0),
        average_speed_mps: Some(4.33),
        average_heart_rate_bpm: None,
        peak_heart_rate_bpm: None,
        average_cadence_rpm: None,
        average_power_watts: None,
        heart_rate_recovery_30_seconds_bpm: None,
        heart_rate_recovery_60_seconds_bpm: None,
        seconds_to_drop_10_bpm: None,
        seconds_to_drop_15_bpm: None,
        summit_immediately_enters_descent: Some(false),
        is_first_half: Some(true),
    }
}
