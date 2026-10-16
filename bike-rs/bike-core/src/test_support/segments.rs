use crate::entities::segments;
pub fn segment() -> segments::Model {
    let at = super::timestamp();
    segments::Model {
        id: 1,
        user_id: 7,
        title: "Fixture segment".into(),
        source: "manual_segment_import".into(),
        mode: "xc".into(),
        original_filename: None,
        format: Some("gpx".into()),
        distance_meters: Some(1000.0),
        starred: false,
        route_data_json: None,
        source_activity_id: None,
        source_start_route_point_index: None,
        source_end_route_point_index: None,
        last_activity_change_at: at,
        created_at: at,
        updated_at: at,
    }
}

pub fn segment_summary() -> crate::entities::segment_summaries::Model {
    let effort = super::segment_effort();
    crate::entities::segment_summaries::Model {
        segment_id: effort.segment_id,
        effort_count: 1,
        leader_user_id: Some(effort.user_id),
        leader_effort_id: Some(effort.id),
        best_duration_seconds: Some(effort.duration_seconds),
        latest_activity_started_at: Some(super::activity().started_at),
        latest_activity_id: Some(effort.activity_id),
        latest_effort_id: Some(effort.id),
        created_at: super::timestamp(),
        updated_at: super::timestamp(),
    }
}

pub fn segment_user_summary() -> crate::entities::segment_user_summaries::Model {
    let effort = super::segment_effort();
    crate::entities::segment_user_summaries::Model {
        id: 1,
        segment_id: effort.segment_id,
        user_id: effort.user_id,
        effort_count: 1,
        personal_best_effort_id: Some(effort.id),
        personal_best_duration_seconds: Some(effort.duration_seconds),
        created_at: super::timestamp(),
        updated_at: super::timestamp(),
    }
}
