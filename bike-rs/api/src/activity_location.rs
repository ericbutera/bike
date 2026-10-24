pub use bike_core::activity_cards::location_from_route_points;
use bike_core::activity_details::{deserialize_derived_activity_data, StoredActivityDerivedData};

pub fn location_from_derived_json(derived_data_json: Option<&StoredActivityDerivedData>) -> Option<String> {
    if let Some(location) = derived_data_json
        .and_then(|data| data.source_metadata())
        .and_then(|metadata| metadata.location.as_ref())
    {
        return Some(location.clone());
    }
    let derived_data = deserialize_derived_activity_data(derived_data_json);
    location_from_route_points(&derived_data.route_points)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::activity_details::ActivityRoutePoint;

    #[test]
    fn manual_location_takes_precedence_and_clearing_restores_route_location() {
        use bike_core::activity_data::ActivitySourceMetadata;
        use bike_core::activity_details::{serialize_derived_activity_data, ActivityDerivedData};
        let recording = ActivityDerivedData {
            route_points: vec![ActivityRoutePoint {
                elapsed_seconds: 0,
                latitude: 45.523,
                longitude: -122.676,
                distance_meters: None,
                elevation_meters: None,
                speed_mps: None,
                heart_rate_bpm: None,
                cadence_rpm: None,
                power_watts: None,
            }],
            ..Default::default()
        };
        let stored = serialize_derived_activity_data(&recording).unwrap();
        let derived_location = location_from_derived_json(Some(&stored));
        assert!(derived_location.is_some());
        let edited = stored.with_source_metadata(ActivitySourceMetadata {
            location: Some("Saved location".into()),
            ..Default::default()
        });
        assert_eq!(
            location_from_derived_json(Some(&edited)).as_deref(),
            Some("Saved location")
        );
        let cleared = edited.with_source_metadata(ActivitySourceMetadata::default());
        assert_eq!(location_from_derived_json(Some(&cleared)), derived_location);
    }
}
