use crate::activity_details::{
    deserialize_derived_activity_data, ActivityRoutePoint, StoredActivityDerivedData,
};
use once_cell::sync::Lazy;
use reverse_geocoder::{Record, ReverseGeocoder};

static REVERSE_GEOCODER: Lazy<ReverseGeocoder> = Lazy::new(ReverseGeocoder::new);

pub fn location_from_derived_json(
    derived_data_json: Option<&StoredActivityDerivedData>,
) -> Option<String> {
    let derived_data = deserialize_derived_activity_data(derived_data_json);
    location_from_route_points(&derived_data.route_points)
}

pub fn location_from_route_points(route_points: &[ActivityRoutePoint]) -> Option<String> {
    let point = route_points.first()?;
    let result = REVERSE_GEOCODER.search((point.latitude, point.longitude));

    format_record(&result.record)
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
