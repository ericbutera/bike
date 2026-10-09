use super::*;

#[test]
fn unit_happy_heatmap_filters_validate_and_identify_the_selected_period() {
    let query = HeatmapQuery {
        sport: Some(ActivitySport::RoadRide),
        from: Some("2026-10-01T00:00:00Z".parse().unwrap()),
        to: Some("2026-11-01T00:00:00Z".parse().unwrap()),
        revision: Some("generation-7".into()),
    };

    assert!(query.validate().is_ok());
    assert_eq!(
        query.cache_key(),
        "road_ride:2026-10-01T00:00:00+00:00:2026-11-01T00:00:00+00:00"
    );
    assert_eq!(HeatmapQuery::default().cache_key(), "all::");
}
