use bike_core::activity_details::{deserialize_derived_activity_data, ActivityRoutePoint, StoredActivityDerivedData};
use once_cell::sync::Lazy;
use reverse_geocoder::{Record, ReverseGeocoder};

static REVERSE_GEOCODER: Lazy<ReverseGeocoder> = Lazy::new(ReverseGeocoder::new);

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

pub fn location_from_route_points(route_points: &[ActivityRoutePoint]) -> Option<String> {
    let point = route_points.first()?;
    let result = REVERSE_GEOCODER.search((point.latitude, point.longitude));

    format_record(result.record)
}

fn format_record(record: &Record) -> Option<String> {
    let name = non_empty(&record.name)?;
    let admin1 = non_empty(&record.admin1);
    let cc = non_empty(&record.cc);

    if let Some(admin1) = admin1.filter(|admin1| !admin1.eq_ignore_ascii_case(name)) {
        return Some(format!("{name}, {admin1}"));
    }

    if let Some(cc) = cc.filter(|cc| !cc.eq_ignore_ascii_case(name)) {
        return Some(format!("{name}, {cc}"));
    }

    Some(name.to_string())
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn formats_city_and_admin1_when_available() {
        let result = format_record(&Record {
            lat: 0.0,
            lon: 0.0,
            name: "Traverse City".to_string(),
            admin1: "MI".to_string(),
            admin2: "Grand Traverse".to_string(),
            cc: "US".to_string(),
        });

        assert_eq!(result.as_deref(), Some("Traverse City, MI"));
    }

    #[test]
    fn falls_back_to_city_and_country_code_when_admin1_is_missing() {
        let result = format_record(&Record {
            lat: 0.0,
            lon: 0.0,
            name: "Reykjavik".to_string(),
            admin1: "".to_string(),
            admin2: "".to_string(),
            cc: "IS".to_string(),
        });

        assert_eq!(result.as_deref(), Some("Reykjavik, IS"));
    }

    #[test]
    fn reverse_geocodes_route_points_to_human_location() {
        let result = location_from_route_points(&[ActivityRoutePoint {
            elapsed_seconds: 0,
            latitude: 45.523,
            longitude: -122.676,
            distance_meters: None,
            elevation_meters: None,
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_watts: None,
        }]);

        assert!(result.is_some());
        assert!(!result.as_deref().unwrap_or_default().contains("122.676"));
    }
}
