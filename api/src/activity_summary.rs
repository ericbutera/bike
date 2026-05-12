use crate::app_error::AppError;
use crate::fit_support::parse_fit_activity;
use chrono::{DateTime, Duration, Utc};
use roxmltree::{Document, Node};
use std::path::Path;

#[derive(Debug, Clone, PartialEq)]
pub struct ActivityDraft {
    pub title: String,
    pub sport: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub distance_meters: Option<f64>,
    pub moving_time_seconds: Option<i32>,
    pub total_time_seconds: Option<i32>,
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
}

#[derive(Debug, Clone)]
struct TrackPointSample {
    lat: Option<f64>,
    lon: Option<f64>,
    elevation_meters: Option<f64>,
    time: Option<DateTime<Utc>>,
    distance_meters: Option<f64>,
    heart_rate_bpm: Option<i32>,
    cadence_rpm: Option<i32>,
}

pub fn summarize_activity_upload(
    filename: &str,
    format: &str,
    bytes: &[u8],
) -> Result<ActivityDraft, AppError> {
    let result = match format {
        "gpx" => parse_gpx_activity(filename, bytes),
        "tcx" => parse_tcx_activity(filename, bytes),
        "fit" => parse_fit_activity(filename, bytes).map(|parsed| parsed.draft),
        _ => Err("Only .fit, .tcx, and .gpx uploads are supported".to_string()),
    };

    result.map_err(|message| AppError::validation_field("file", message))
}

fn parse_gpx_activity(filename: &str, bytes: &[u8]) -> Result<ActivityDraft, String> {
    let document = parse_xml_document(bytes, "GPX")?;
    let track = document
        .descendants()
        .find(|node| is_element_named(*node, "trk"))
        .ok_or_else(|| "GPX file is missing a <trk> track element".to_string())?;

    let points = track
        .descendants()
        .filter(|node| is_element_named(*node, "trkpt"))
        .map(parse_gpx_track_point)
        .collect::<Result<Vec<_>, _>>()?;

    if points.is_empty() {
        return Err("GPX file does not contain any track points".to_string());
    }

    let metadata_time = document
        .descendants()
        .find(|node| is_element_named(*node, "metadata"))
        .and_then(|node| child_text(node, "time"))
        .and_then(parse_datetime);
    let first_time = points.iter().find_map(|point| point.time);
    let started_at = first_time.or(metadata_time).unwrap_or_else(Utc::now);
    let ended_at = points.iter().rev().find_map(|point| point.time);
    let total_time_seconds = ended_at.and_then(|end| seconds_between(started_at, end));
    let (distance_meters, max_speed_mps) = summarize_geospatial_distance(&points);
    let (elevation_gain_meters, elevation_loss_meters) = summarize_elevation(&points);
    let heart_rates = points
        .iter()
        .filter_map(|point| point.heart_rate_bpm)
        .collect::<Vec<_>>();
    let cadences = points
        .iter()
        .filter_map(|point| point.cadence_rpm)
        .collect::<Vec<_>>();
    let average_speed_mps = match (distance_meters, total_time_seconds) {
        (Some(distance), Some(total_seconds)) if total_seconds > 0 => {
            Some(distance / f64::from(total_seconds))
        }
        _ => None,
    };
    let title = child_text(track, "name")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| humanize_filename(filename));
    let sport = normalize_sport(child_text(track, "type"));

    Ok(ActivityDraft {
        title,
        sport,
        started_at,
        ended_at,
        distance_meters,
        moving_time_seconds: total_time_seconds,
        total_time_seconds,
        elevation_gain_meters,
        elevation_loss_meters,
        average_speed_mps,
        max_speed_mps,
        average_heart_rate_bpm: average_metric(&heart_rates),
        max_heart_rate_bpm: max_metric(&heart_rates),
        average_cadence_rpm: average_metric(&cadences),
        max_cadence_rpm: max_metric(&cadences),
        calories: None,
    })
}

fn parse_tcx_activity(filename: &str, bytes: &[u8]) -> Result<ActivityDraft, String> {
    let document = parse_xml_document(bytes, "TCX")?;
    let activity = document
        .descendants()
        .find(|node| is_element_named(*node, "Activity"))
        .ok_or_else(|| "TCX file is missing an <Activity> element".to_string())?;
    let laps = activity
        .children()
        .filter(|node| is_element_named(*node, "Lap"))
        .collect::<Vec<_>>();
    let points = activity
        .descendants()
        .filter(|node| is_element_named(*node, "Trackpoint"))
        .map(parse_tcx_track_point)
        .collect::<Result<Vec<_>, _>>()?;

    if laps.is_empty() && points.is_empty() {
        return Err("TCX file does not contain any laps or track points".to_string());
    }

    let mut lap_total_seconds = 0.0;
    let mut lap_total_distance_meters = 0.0;
    let mut lap_calories = 0i32;
    let mut calories_found = false;
    let mut weighted_lap_heart_rate_sum = 0.0;
    let mut weighted_lap_heart_rate_seconds = 0.0;
    let mut weighted_lap_cadence_sum = 0.0;
    let mut weighted_lap_cadence_seconds = 0.0;
    let mut lap_max_speed_mps = None;
    let mut lap_max_heart_rate_bpm = None;

    for lap in &laps {
        let lap_seconds = child_text(*lap, "TotalTimeSeconds")
            .and_then(parse_f64)
            .unwrap_or(0.0)
            .max(0.0);
        lap_total_seconds += lap_seconds;

        if let Some(distance) = child_text(*lap, "DistanceMeters").and_then(parse_f64) {
            lap_total_distance_meters += distance.max(0.0);
        }

        if let Some(calories) = child_text(*lap, "Calories").and_then(parse_i32) {
            calories_found = true;
            lap_calories += calories.max(0);
        }

        update_max_option(
            &mut lap_max_speed_mps,
            child_text(*lap, "MaximumSpeed").and_then(parse_f64),
        );
        update_max_option(
            &mut lap_max_heart_rate_bpm,
            child_element(*lap, "MaximumHeartRateBpm")
                .and_then(|node| child_text(node, "Value"))
                .and_then(parse_i32),
        );

        if let Some(average_heart_rate) = child_element(*lap, "AverageHeartRateBpm")
            .and_then(|node| child_text(node, "Value"))
            .and_then(parse_i32)
        {
            weighted_lap_heart_rate_sum += f64::from(average_heart_rate) * lap_seconds;
            weighted_lap_heart_rate_seconds += lap_seconds;
        }

        if let Some(cadence) = child_text(*lap, "Cadence").and_then(parse_i32) {
            weighted_lap_cadence_sum += f64::from(cadence) * lap_seconds;
            weighted_lap_cadence_seconds += lap_seconds;
        }
    }

    let started_at = child_text(activity, "Id")
        .and_then(parse_datetime)
        .or_else(|| points.iter().find_map(|point| point.time))
        .unwrap_or_else(Utc::now);
    let ended_at = points
        .iter()
        .rev()
        .find_map(|point| point.time)
        .or_else(|| {
            seconds_from_f64(lap_total_seconds)
                .map(|seconds| started_at + Duration::seconds(i64::from(seconds)))
        });
    let total_time_seconds = seconds_from_f64(lap_total_seconds)
        .or_else(|| ended_at.and_then(|end| seconds_between(started_at, end)));
    let fallback_distance_meters = points
        .iter()
        .filter_map(|point| point.distance_meters)
        .reduce(f64::max);
    let distance_meters = metric_from_f64(if lap_total_distance_meters > 0.0 {
        Some(lap_total_distance_meters)
    } else {
        fallback_distance_meters
    });
    let (elevation_gain_meters, elevation_loss_meters) = summarize_elevation(&points);
    let point_heart_rates = points
        .iter()
        .filter_map(|point| point.heart_rate_bpm)
        .collect::<Vec<_>>();
    let point_cadences = points
        .iter()
        .filter_map(|point| point.cadence_rpm)
        .collect::<Vec<_>>();
    let average_heart_rate_bpm = average_metric(&point_heart_rates).or_else(|| {
        if weighted_lap_heart_rate_seconds > 0.0 {
            Some((weighted_lap_heart_rate_sum / weighted_lap_heart_rate_seconds).round() as i32)
        } else {
            None
        }
    });
    let max_heart_rate_bpm = max_metric(&point_heart_rates).or(lap_max_heart_rate_bpm);
    let average_cadence_rpm = average_metric(&point_cadences).or_else(|| {
        if weighted_lap_cadence_seconds > 0.0 {
            Some((weighted_lap_cadence_sum / weighted_lap_cadence_seconds).round() as i32)
        } else {
            None
        }
    });
    let max_cadence_rpm = max_metric(&point_cadences);
    let point_max_speed_mps = summarize_distance_samples(&points);
    let average_speed_mps = match (distance_meters, total_time_seconds) {
        (Some(distance), Some(total_seconds)) if total_seconds > 0 => {
            Some(distance / f64::from(total_seconds))
        }
        _ => None,
    };
    let calories = if calories_found {
        Some(lap_calories)
    } else {
        None
    };

    Ok(ActivityDraft {
        title: humanize_filename(filename),
        sport: normalize_sport(activity.attribute("Sport")),
        started_at,
        ended_at,
        distance_meters,
        moving_time_seconds: total_time_seconds,
        total_time_seconds,
        elevation_gain_meters,
        elevation_loss_meters,
        average_speed_mps,
        max_speed_mps: merge_max(point_max_speed_mps, lap_max_speed_mps),
        average_heart_rate_bpm,
        max_heart_rate_bpm: merge_max(max_heart_rate_bpm, lap_max_heart_rate_bpm),
        average_cadence_rpm,
        max_cadence_rpm,
        calories,
    })
}

fn parse_xml_document<'a>(bytes: &'a [u8], format_name: &str) -> Result<Document<'a>, String> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|_| format!("{format_name} uploads must be UTF-8 XML files"))?;
    let xml = xml
        .strip_prefix('\u{feff}')
        .unwrap_or(xml)
        .trim_start_matches(|character: char| character.is_whitespace());
    Document::parse(xml).map_err(|err| format!("Failed to parse {format_name} file: {err}"))
}

fn parse_gpx_track_point(node: Node<'_, '_>) -> Result<TrackPointSample, String> {
    let lat = node
        .attribute("lat")
        .and_then(parse_f64)
        .ok_or_else(|| "GPX track point is missing a valid latitude".to_string())?;
    let lon = node
        .attribute("lon")
        .and_then(parse_f64)
        .ok_or_else(|| "GPX track point is missing a valid longitude".to_string())?;

    Ok(TrackPointSample {
        lat: Some(lat),
        lon: Some(lon),
        elevation_meters: child_text(node, "ele").and_then(parse_f64),
        time: child_text(node, "time").and_then(parse_datetime),
        distance_meters: None,
        heart_rate_bpm: descendant_text(node, "hr").as_deref().and_then(parse_i32),
        cadence_rpm: descendant_text(node, "cad").as_deref().and_then(parse_i32),
    })
}

fn parse_tcx_track_point(node: Node<'_, '_>) -> Result<TrackPointSample, String> {
    Ok(TrackPointSample {
        lat: None,
        lon: None,
        elevation_meters: child_text(node, "AltitudeMeters").and_then(parse_f64),
        time: child_text(node, "Time").and_then(parse_datetime),
        distance_meters: child_text(node, "DistanceMeters").and_then(parse_f64),
        heart_rate_bpm: child_element(node, "HeartRateBpm")
            .and_then(|heart_rate| child_text(heart_rate, "Value"))
            .and_then(parse_i32),
        cadence_rpm: child_text(node, "Cadence").and_then(parse_i32).or_else(|| {
            descendant_text(node, "RunCadence")
                .as_deref()
                .and_then(parse_i32)
        }),
    })
}

fn summarize_geospatial_distance(points: &[TrackPointSample]) -> (Option<f64>, Option<f64>) {
    let mut distance_meters = 0.0;
    let mut max_speed_mps = None;

    for window in points.windows(2) {
        let previous = &window[0];
        let current = &window[1];
        let segment_distance_meters = match (previous.lat, previous.lon, current.lat, current.lon) {
            (Some(prev_lat), Some(prev_lon), Some(curr_lat), Some(curr_lon)) => {
                haversine_distance_meters(prev_lat, prev_lon, curr_lat, curr_lon)
            }
            _ => 0.0,
        };
        distance_meters += segment_distance_meters;

        if let (Some(start), Some(end)) = (previous.time, current.time) {
            if let Some(segment_seconds) = seconds_between(start, end) {
                if segment_seconds > 0 {
                    update_max_option(
                        &mut max_speed_mps,
                        Some(segment_distance_meters / f64::from(segment_seconds)),
                    );
                }
            }
        }
    }

    (
        metric_from_f64(Some(distance_meters)),
        metric_from_f64(max_speed_mps),
    )
}

fn summarize_distance_samples(points: &[TrackPointSample]) -> Option<f64> {
    let mut max_speed_mps = None;

    for window in points.windows(2) {
        let previous = &window[0];
        let current = &window[1];

        if let (Some(prev_distance), Some(curr_distance), Some(start), Some(end)) = (
            previous.distance_meters,
            current.distance_meters,
            previous.time,
            current.time,
        ) {
            if let Some(segment_seconds) = seconds_between(start, end) {
                if segment_seconds > 0 && curr_distance >= prev_distance {
                    update_max_option(
                        &mut max_speed_mps,
                        Some((curr_distance - prev_distance) / f64::from(segment_seconds)),
                    );
                }
            }
        }
    }

    metric_from_f64(max_speed_mps)
}

fn summarize_elevation(points: &[TrackPointSample]) -> (Option<f64>, Option<f64>) {
    let mut gain_meters = 0.0;
    let mut loss_meters = 0.0;

    for window in points.windows(2) {
        if let (Some(previous), Some(current)) =
            (window[0].elevation_meters, window[1].elevation_meters)
        {
            let delta = current - previous;
            if delta > 0.0 {
                gain_meters += delta;
            } else {
                loss_meters += delta.abs();
            }
        }
    }

    (
        metric_from_f64(Some(gain_meters)),
        metric_from_f64(Some(loss_meters)),
    )
}

fn average_metric(values: &[i32]) -> Option<i32> {
    if values.is_empty() {
        return None;
    }

    Some(
        (values.iter().copied().map(i64::from).sum::<i64>() as f64 / values.len() as f64).round()
            as i32,
    )
}

fn max_metric(values: &[i32]) -> Option<i32> {
    values.iter().copied().max()
}

fn metric_from_f64(value: Option<f64>) -> Option<f64> {
    value
        .filter(|metric| metric.is_finite())
        .filter(|metric| *metric > 0.0)
}

fn seconds_from_f64(value: f64) -> Option<i32> {
    if value.is_finite() && value > 0.0 && value <= f64::from(i32::MAX) {
        Some(value.round() as i32)
    } else {
        None
    }
}

fn seconds_between(start: DateTime<Utc>, end: DateTime<Utc>) -> Option<i32> {
    let seconds = (end - start).num_seconds();
    if seconds > 0 && seconds <= i64::from(i32::MAX) {
        Some(seconds as i32)
    } else {
        None
    }
}

fn parse_f64(value: &str) -> Option<f64> {
    value.trim().parse::<f64>().ok()
}

fn parse_i32(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok()
}

fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value.trim())
        .ok()
        .map(|datetime| datetime.with_timezone(&Utc))
}

fn humanize_filename(filename: &str) -> String {
    let raw = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("activity")
        .replace(['_', '-'], " ");
    let compact = raw.split_whitespace().collect::<Vec<_>>().join(" ");

    if compact.is_empty() {
        "Activity".to_string()
    } else {
        title_case_words(&compact)
    }
}

fn title_case_words(value: &str) -> String {
    value
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => {
                    first.to_uppercase().collect::<String>() + &chars.as_str().to_ascii_lowercase()
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_sport(value: Option<&str>) -> String {
    match value
        .unwrap_or("activity")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "bike" | "biking" | "cycling" | "ride" => "ride".to_string(),
        "run" | "running" => "run".to_string(),
        "swim" | "swimming" => "swim".to_string(),
        "" => "activity".to_string(),
        other => other.to_string(),
    }
}

fn haversine_distance_meters(
    start_latitude: f64,
    start_longitude: f64,
    end_latitude: f64,
    end_longitude: f64,
) -> f64 {
    let earth_radius_meters = 6_371_000.0;
    let latitude_delta = (end_latitude - start_latitude).to_radians();
    let longitude_delta = (end_longitude - start_longitude).to_radians();
    let start_latitude_radians = start_latitude.to_radians();
    let end_latitude_radians = end_latitude.to_radians();
    let a = (latitude_delta / 2.0).sin().powi(2)
        + start_latitude_radians.cos()
            * end_latitude_radians.cos()
            * (longitude_delta / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());

    earth_radius_meters * c
}

fn update_max_option<T>(current: &mut Option<T>, candidate: Option<T>)
where
    T: PartialOrd + Copy,
{
    if let Some(candidate) = candidate {
        match current {
            Some(existing) if *existing >= candidate => {}
            _ => *current = Some(candidate),
        }
    }
}

fn merge_max<T>(left: Option<T>, right: Option<T>) -> Option<T>
where
    T: PartialOrd + Copy,
{
    let mut merged = left;
    update_max_option(&mut merged, right);
    merged
}

fn child_element<'a>(node: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    node.children().find(|child| is_element_named(*child, name))
}

fn child_text<'a>(node: Node<'a, 'a>, name: &str) -> Option<&'a str> {
    child_element(node, name)
        .and_then(|child| child.text())
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn descendant_text(node: Node<'_, '_>, name: &str) -> Option<String> {
    node.descendants()
        .find(|descendant| is_element_named(*descendant, name))
        .and_then(|descendant| descendant.text())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn is_element_named(node: Node<'_, '_>, name: &str) -> bool {
    node.is_element() && node.tag_name().name() == name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_gpx_uploads_into_activity_metrics() {
        let gpx = r#"
            <gpx version="1.1" creator="Bike"
                xmlns="http://www.topografix.com/GPX/1/1"
                xmlns:gpxtpx="http://www.garmin.com/xmlschemas/TrackPointExtension/v1">
              <trk>
                <name>Evening Ride</name>
                <type>Ride</type>
                <trkseg>
                  <trkpt lat="35.0000" lon="-120.0000">
                    <ele>10</ele>
                    <time>2026-05-01T10:00:00Z</time>
                    <extensions>
                      <gpxtpx:TrackPointExtension>
                        <gpxtpx:hr>120</gpxtpx:hr>
                        <gpxtpx:cad>80</gpxtpx:cad>
                      </gpxtpx:TrackPointExtension>
                    </extensions>
                  </trkpt>
                  <trkpt lat="35.0005" lon="-120.0005">
                    <ele>14</ele>
                    <time>2026-05-01T10:05:00Z</time>
                    <extensions>
                      <gpxtpx:TrackPointExtension>
                        <gpxtpx:hr>140</gpxtpx:hr>
                        <gpxtpx:cad>90</gpxtpx:cad>
                      </gpxtpx:TrackPointExtension>
                    </extensions>
                  </trkpt>
                  <trkpt lat="35.0010" lon="-120.0010">
                    <ele>12</ele>
                    <time>2026-05-01T10:10:00Z</time>
                    <extensions>
                      <gpxtpx:TrackPointExtension>
                        <gpxtpx:hr>130</gpxtpx:hr>
                        <gpxtpx:cad>85</gpxtpx:cad>
                      </gpxtpx:TrackPointExtension>
                    </extensions>
                  </trkpt>
                </trkseg>
              </trk>
            </gpx>
        "#;

        let summary = summarize_activity_upload("evening-ride.gpx", "gpx", gpx.as_bytes())
            .expect("gpx summary");

        assert_eq!(summary.title, "Evening Ride");
        assert_eq!(summary.sport, "ride");
        assert_eq!(summary.total_time_seconds, Some(600));
        assert_eq!(summary.average_heart_rate_bpm, Some(130));
        assert_eq!(summary.max_heart_rate_bpm, Some(140));
        assert_eq!(summary.average_cadence_rpm, Some(85));
        assert_eq!(summary.max_cadence_rpm, Some(90));
        assert_eq!(summary.elevation_gain_meters, Some(4.0));
        assert_eq!(summary.elevation_loss_meters, Some(2.0));
        assert!(summary.distance_meters.unwrap_or_default() > 100.0);
        assert!(summary.average_speed_mps.unwrap_or_default() > 0.0);
    }

    #[test]
    fn summarizes_tcx_uploads_into_activity_metrics() {
        let tcx = r#"
            <TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2">
              <Activities>
                <Activity Sport="Biking">
                  <Id>2026-05-01T12:00:00Z</Id>
                  <Lap StartTime="2026-05-01T12:00:00Z">
                    <TotalTimeSeconds>600</TotalTimeSeconds>
                    <DistanceMeters>5000</DistanceMeters>
                    <MaximumSpeed>12.5</MaximumSpeed>
                    <Calories>220</Calories>
                    <AverageHeartRateBpm>
                      <Value>135</Value>
                    </AverageHeartRateBpm>
                    <MaximumHeartRateBpm>
                      <Value>158</Value>
                    </MaximumHeartRateBpm>
                    <Cadence>88</Cadence>
                    <Track>
                      <Trackpoint>
                        <Time>2026-05-01T12:00:00Z</Time>
                        <AltitudeMeters>100</AltitudeMeters>
                        <DistanceMeters>0</DistanceMeters>
                        <HeartRateBpm><Value>130</Value></HeartRateBpm>
                        <Cadence>82</Cadence>
                      </Trackpoint>
                      <Trackpoint>
                        <Time>2026-05-01T12:05:00Z</Time>
                        <AltitudeMeters>118</AltitudeMeters>
                        <DistanceMeters>2500</DistanceMeters>
                        <HeartRateBpm><Value>140</Value></HeartRateBpm>
                        <Cadence>88</Cadence>
                      </Trackpoint>
                      <Trackpoint>
                        <Time>2026-05-01T12:10:00Z</Time>
                        <AltitudeMeters>110</AltitudeMeters>
                        <DistanceMeters>5000</DistanceMeters>
                        <HeartRateBpm><Value>150</Value></HeartRateBpm>
                        <Cadence>92</Cadence>
                      </Trackpoint>
                    </Track>
                  </Lap>
                </Activity>
              </Activities>
            </TrainingCenterDatabase>
        "#;

        let summary = summarize_activity_upload("lunch-ride.tcx", "tcx", tcx.as_bytes())
            .expect("tcx summary");

        assert_eq!(summary.title, "Lunch Ride");
        assert_eq!(summary.sport, "ride");
        assert_eq!(summary.total_time_seconds, Some(600));
        assert_eq!(summary.distance_meters, Some(5000.0));
        assert_eq!(summary.calories, Some(220));
        assert_eq!(summary.average_heart_rate_bpm, Some(140));
        assert_eq!(summary.max_heart_rate_bpm, Some(158));
        assert_eq!(summary.average_cadence_rpm, Some(87));
        assert_eq!(summary.max_cadence_rpm, Some(92));
        assert_eq!(summary.elevation_gain_meters, Some(18.0));
        assert_eq!(summary.elevation_loss_meters, Some(8.0));
        assert_eq!(summary.max_speed_mps, Some(12.5));
    }

    #[test]
    fn summarizes_tcx_uploads_with_leading_whitespace_before_xml_declaration() {
        let tcx = "        <?xml version=\"1.0\" encoding=\"utf-8\"?>\n        <TrainingCenterDatabase xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\">\n          <Activities>\n            <Activity Sport=\"Biking\">\n              <Id>2026-05-01T10:00:00Z</Id>\n              <Lap StartTime=\"2026-05-01T10:00:00Z\">\n                <TotalTimeSeconds>300</TotalTimeSeconds>\n                <DistanceMeters>1200</DistanceMeters>\n                <Track>\n                  <Trackpoint>\n                    <Time>2026-05-01T10:00:00Z</Time>\n                    <Position>\n                      <LatitudeDegrees>45.0</LatitudeDegrees>\n                      <LongitudeDegrees>-122.0</LongitudeDegrees>\n                    </Position>\n                    <AltitudeMeters>10</AltitudeMeters>\n                    <DistanceMeters>0</DistanceMeters>\n                  </Trackpoint>\n                  <Trackpoint>\n                    <Time>2026-05-01T10:05:00Z</Time>\n                    <Position>\n                      <LatitudeDegrees>45.01</LatitudeDegrees>\n                      <LongitudeDegrees>-122.01</LongitudeDegrees>\n                    </Position>\n                    <AltitudeMeters>15</AltitudeMeters>\n                    <DistanceMeters>1200</DistanceMeters>\n                  </Trackpoint>\n                </Track>\n              </Lap>\n            </Activity>\n          </Activities>\n        </TrainingCenterDatabase>\n";

        let summary = summarize_activity_upload("whitespace.tcx", "tcx", tcx.as_bytes())
            .expect("tcx summary with leading whitespace");

        assert_eq!(summary.distance_meters, Some(1200.0));
        assert_eq!(summary.total_time_seconds, Some(300));
    }

    #[test]
    fn summarizes_fit_uploads_into_activity_metrics() {
        let fit = include_bytes!("../tests/fixtures/activity.fit");
        let summary = summarize_activity_upload("activity.fit", "fit", fit)
            .expect("fit summary");

        assert_eq!(summary.title, "Activity");
        assert_eq!(summary.sport, "run");
        assert_eq!(summary.total_time_seconds, Some(14));
        assert_eq!(summary.moving_time_seconds, Some(14));
        assert_eq!(summary.distance_meters, Some(5.73));
        assert_eq!(summary.elevation_gain_meters, Some(0.0));
        assert_eq!(summary.elevation_loss_meters, Some(0.0));
    }
}
