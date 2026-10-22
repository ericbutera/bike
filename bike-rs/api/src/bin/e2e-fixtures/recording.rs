use super::competition::{FixtureResult, Ride};
use bike_core::activity_data::ActivityRoutePoint;
use bike_core::activity_parser::parse_activity_data;
use chrono::Duration;
use std::fmt::Write;

pub struct Routes {
    city: Vec<ActivityRoutePoint>,
    unmarked: Vec<ActivityRoutePoint>,
}

impl Routes {
    pub fn load() -> FixtureResult<Self> {
        Ok(Self {
            city: parse_activity_data(
                "city_02.fit",
                "fit",
                include_bytes!("../../../tests/fixtures/segment_matching/activities/city_02.fit"),
            )?
            .derived_data
            .route_points,
            unmarked: parse_activity_data(
                "unmarked_01.fit",
                "fit",
                include_bytes!("../../../tests/fixtures/segment_matching/activities/unmarked_01.fit"),
            )?
            .derived_data
            .route_points,
        })
    }

    pub fn gpx(&self, ride: &Ride) -> FixtureResult<Vec<u8>> {
        let route = match ride.route.as_str() {
            "city" => &self.city,
            "unmarked" => &self.unmarked,
            _ => return Err("Unknown competition route".into()),
        };
        if !(0.5..=3.0).contains(&ride.pace) || !(1..=3).contains(&ride.repeats) {
            return Err("Competition pace/repeats are outside the fixture range".into());
        }
        let mut xml = String::from(
            "<?xml version=\"1.0\"?><gpx version=\"1.1\" creator=\"Bike fictional competition fixture\" xmlns=\"http://www.topografix.com/GPX/1/1\" xmlns:gpxtpx=\"http://www.garmin.com/xmlschemas/TrackPointExtension/v1\"><trk>",
        );
        write!(
            xml,
            "<name>Fixture {} rider {} ride {}</name>",
            ride.route, ride.user_id, ride.id
        )?;
        xml.push_str("<type>cycling</type>");
        let mut offset = 0;
        for attempt in 0..ride.repeats {
            xml.push_str("<trkseg>");
            let pace = ride.pace + f64::from(attempt) * 0.03;
            for point in route {
                let milliseconds = (f64::from(point.elapsed_seconds) * pace * 1000.0).round() as i64;
                let time = ride.started_at + Duration::milliseconds(offset + milliseconds);
                write!(
                    xml,
                    "<trkpt lat=\"{}\" lon=\"{}\"><time>{}</time>",
                    point.latitude,
                    point.longitude,
                    time.to_rfc3339()
                )?;
                write_samples(&mut xml, point)?;
                xml.push_str("</trkpt>");
            }
            xml.push_str("</trkseg>");
            let duration = route.last().ok_or("Fixture route is empty")?.elapsed_seconds;
            offset += (f64::from(duration) * pace * 1000.0).round() as i64 + 60_000;
        }
        xml.push_str("</trk></gpx>");
        Ok(xml.into_bytes())
    }
}

fn write_samples(xml: &mut String, point: &ActivityRoutePoint) -> std::fmt::Result {
    if let Some(elevation) = point.elevation_meters {
        write!(xml, "<ele>{elevation}</ele>")?;
    }
    xml.push_str("<extensions><gpxtpx:TrackPointExtension>");
    if let Some(hr) = point.heart_rate_bpm {
        write!(xml, "<gpxtpx:hr>{hr}</gpxtpx:hr>")?;
    }
    if let Some(cadence) = point.cadence_rpm {
        write!(xml, "<gpxtpx:cad>{cadence}</gpxtpx:cad>")?;
    }
    xml.push_str("</gpxtpx:TrackPointExtension></extensions>");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ride(route: &str) -> Ride {
        Ride {
            id: 31001,
            user_id: 1,
            route: route.into(),
            started_at: "2025-06-01T08:00:00Z".parse().unwrap(),
            pace: 1.0,
            repeats: 1,
        }
    }

    #[test]
    fn real_routes_produce_reparseable_fictional_recordings() {
        let routes = Routes::load().unwrap();
        for (name, source) in [("city", &routes.city), ("unmarked", &routes.unmarked)] {
            let mut ride = ride(name);
            ride.pace = 1.5;
            let bytes = routes.gpx(&ride).unwrap();
            let parsed = parse_activity_data("competition.gpx", "gpx", &bytes).unwrap();
            assert_eq!(parsed.draft.started_at, ride.started_at);
            assert!(bike_core::activity_sport::is_bike_activity_sport(&parsed.draft.sport));
            assert!(parsed.draft.title.starts_with("Fixture"));
            assert_eq!(parsed.derived_data.route_points.len(), source.len());
            for (actual, original) in parsed.derived_data.route_points.iter().zip(source) {
                assert_eq!(
                    (actual.latitude, actual.longitude),
                    (original.latitude, original.longitude)
                );
                let expected = f64::from(original.elapsed_seconds) * ride.pace;
                assert!((f64::from(actual.elapsed_seconds) - expected).abs() <= 1.0);
            }
            assert!(parsed.draft.distance_meters.unwrap() > 1000.0);
            assert!(parsed.draft.average_heart_rate_bpm.is_some());
        }
    }

    #[test]
    fn repeated_route_has_three_distinct_recording_windows() {
        let routes = Routes::load().unwrap();
        let mut ride = ride("city");
        ride.repeats = 3;
        let bytes = routes.gpx(&ride).unwrap();
        let parsed = parse_activity_data("repeats.gpx", "gpx", &bytes).unwrap();
        let points = parsed.derived_data.route_points;
        assert_eq!(points.len(), routes.city.len() * 3);
        assert!(points
            .windows(2)
            .all(|pair| pair[1].elapsed_seconds >= pair[0].elapsed_seconds));
        for attempt in 1..3 {
            let start = attempt * routes.city.len();
            assert_eq!(points[start].latitude, points[0].latitude);
            assert_eq!(points[start].elapsed_seconds - points[start - 1].elapsed_seconds, 60);
        }
    }

    #[test]
    fn unknown_routes_and_invalid_timing_are_rejected() {
        let routes = Routes::load().unwrap();
        assert!(routes.gpx(&ride("unknown")).is_err());
        let mut ride = ride("city");
        ride.pace = f64::NAN;
        assert!(routes.gpx(&ride).is_err());
        ride.pace = 1.0;
        ride.repeats = 0;
        assert!(routes.gpx(&ride).is_err());
    }
}
