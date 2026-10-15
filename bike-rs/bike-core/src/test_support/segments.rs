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
