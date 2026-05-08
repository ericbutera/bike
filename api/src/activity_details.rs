use crate::activity_summary::{summarize_activity_upload, ActivityDraft};
use crate::app_error::AppError;
use chrono::{DateTime, Utc};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const MAX_CHART_POINTS: usize = 180;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityDerivedData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityLap {
    pub lap_index: i32,
    pub title: String,
    pub start_offset_seconds: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub distance_meters: Option<f64>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityChartPoint {
    pub elapsed_seconds: i32,
    pub distance_meters: Option<f64>,
    pub elevation_meters: Option<f64>,
    pub speed_mps: Option<f64>,
    pub heart_rate_bpm: Option<i32>,
    pub cadence_rpm: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityRoutePoint {
    pub elapsed_seconds: i32,
    pub latitude: f64,
    pub longitude: f64,
    pub distance_meters: Option<f64>,
    pub elevation_meters: Option<f64>,
    pub speed_mps: Option<f64>,
    pub heart_rate_bpm: Option<i32>,
    pub cadence_rpm: Option<i32>,
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

pub fn derive_activity_detail_data(
    filename: &str,
    format: &str,
    bytes: &[u8],
) -> Result<ActivityDerivedData, AppError> {
    let result = match format {
        "gpx" => derive_gpx_activity_detail(filename, bytes),
        "tcx" => derive_tcx_activity_detail(filename, bytes),
        "fit" => Ok(ActivityDerivedData::default()),
        _ => Err("Only .fit, .tcx, and .gpx uploads are supported".to_string()),
    };

    result.map_err(|message| AppError::validation_field("file", message))
}

pub fn serialize_derived_activity_data(data: &ActivityDerivedData) -> Result<String, AppError> {
    serde_json::to_string(data).map_err(|error| {
        tracing::error!(error = ?error, "failed to serialize activity derived data");
        AppError::internal("Failed to serialize activity derived data")
    })
}

pub fn deserialize_derived_activity_data(raw: Option<&str>) -> ActivityDerivedData {
    match raw {
        Some(value) if !value.trim().is_empty() => {
            serde_json::from_str(value).unwrap_or_else(|error| {
                tracing::warn!(error = ?error, "failed to deserialize activity derived data");
                ActivityDerivedData::default()
            })
        }
        _ => ActivityDerivedData::default(),
    }
}

fn derive_gpx_activity_detail(filename: &str, bytes: &[u8]) -> Result<ActivityDerivedData, String> {
    let summary =
        summarize_activity_upload(filename, "gpx", bytes).map_err(|error| error.message)?;
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

    Ok(ActivityDerivedData {
        laps: vec![full_activity_lap(&summary)],
        chart_points: downsample_chart_points(
            build_gpx_chart_points(&points, summary.started_at),
            MAX_CHART_POINTS,
        ),
        route_points: build_gpx_route_points(&points, summary.started_at),
    })
}

fn derive_tcx_activity_detail(filename: &str, bytes: &[u8]) -> Result<ActivityDerivedData, String> {
    let summary =
        summarize_activity_upload(filename, "tcx", bytes).map_err(|error| error.message)?;
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

    let mut detail_laps = Vec::new();
    let mut fallback_start_offset_seconds = 0;

    for (index, lap) in laps.iter().enumerate() {
        let lap_points = lap
            .descendants()
            .filter(|node| is_element_named(*node, "Trackpoint"))
            .map(parse_tcx_track_point)
            .collect::<Result<Vec<_>, _>>()?;
        let lap_duration_seconds = child_text(*lap, "TotalTimeSeconds")
            .and_then(parse_f64)
            .and_then(seconds_from_f64);
        let lap_distance_meters =
            metric_from_f64(child_text(*lap, "DistanceMeters").and_then(parse_f64)).or_else(|| {
                lap_points
                    .iter()
                    .filter_map(|point| point.distance_meters)
                    .reduce(f64::max)
                    .and_then(|distance| metric_from_f64(Some(distance)))
            });
        let (lap_elevation_gain_meters, lap_elevation_loss_meters) =
            summarize_elevation(&lap_points);
        let lap_heart_rates = lap_points
            .iter()
            .filter_map(|point| point.heart_rate_bpm)
            .collect::<Vec<_>>();
        let lap_cadences = lap_points
            .iter()
            .filter_map(|point| point.cadence_rpm)
            .collect::<Vec<_>>();
        let lap_average_heart_rate_bpm = average_metric(&lap_heart_rates).or_else(|| {
            child_element(*lap, "AverageHeartRateBpm")
                .and_then(|node| child_text(node, "Value"))
                .and_then(parse_i32)
        });
        let lap_max_heart_rate_bpm = merge_max(
            max_metric(&lap_heart_rates),
            child_element(*lap, "MaximumHeartRateBpm")
                .and_then(|node| child_text(node, "Value"))
                .and_then(parse_i32),
        );
        let lap_average_cadence_rpm = average_metric(&lap_cadences)
            .or_else(|| child_text(*lap, "Cadence").and_then(parse_i32));
        let lap_max_cadence_rpm = max_metric(&lap_cadences);
        let lap_average_speed_mps = match (lap_distance_meters, lap_duration_seconds) {
            (Some(distance), Some(duration)) if duration > 0 => {
                Some(distance / f64::from(duration))
            }
            _ => None,
        };
        let lap_max_speed_mps = merge_max(
            summarize_distance_samples(&lap_points),
            child_text(*lap, "MaximumSpeed").and_then(parse_f64),
        );
        let start_offset_seconds = lap
            .attribute("StartTime")
            .and_then(parse_datetime)
            .and_then(|lap_start| elapsed_seconds_from(summary.started_at, lap_start))
            .or(Some(fallback_start_offset_seconds));

        detail_laps.push(ActivityLap {
            lap_index: (index + 1) as i32,
            title: format!("Lap {}", index + 1),
            start_offset_seconds,
            duration_seconds: lap_duration_seconds,
            distance_meters: lap_distance_meters,
            elevation_gain_meters: lap_elevation_gain_meters,
            elevation_loss_meters: lap_elevation_loss_meters,
            average_speed_mps: lap_average_speed_mps,
            max_speed_mps: lap_max_speed_mps,
            average_heart_rate_bpm: lap_average_heart_rate_bpm,
            max_heart_rate_bpm: lap_max_heart_rate_bpm,
            average_cadence_rpm: lap_average_cadence_rpm,
            max_cadence_rpm: lap_max_cadence_rpm,
            calories: child_text(*lap, "Calories").and_then(parse_i32),
        });

        fallback_start_offset_seconds =
            fallback_start_offset_seconds.saturating_add(lap_duration_seconds.unwrap_or_default());
    }

    if detail_laps.is_empty() {
        detail_laps.push(full_activity_lap(&summary));
    }

    Ok(ActivityDerivedData {
        laps: detail_laps,
        chart_points: downsample_chart_points(
            build_tcx_chart_points(&points, summary.started_at),
            MAX_CHART_POINTS,
        ),
        route_points: build_tcx_route_points(&points, summary.started_at),
    })
}

fn full_activity_lap(summary: &ActivityDraft) -> ActivityLap {
    ActivityLap {
        lap_index: 1,
        title: "Full activity".to_string(),
        start_offset_seconds: Some(0),
        duration_seconds: summary.total_time_seconds,
        distance_meters: summary.distance_meters,
        elevation_gain_meters: summary.elevation_gain_meters,
        elevation_loss_meters: summary.elevation_loss_meters,
        average_speed_mps: summary.average_speed_mps,
        max_speed_mps: summary.max_speed_mps,
        average_heart_rate_bpm: summary.average_heart_rate_bpm,
        max_heart_rate_bpm: summary.max_heart_rate_bpm,
        average_cadence_rpm: summary.average_cadence_rpm,
        max_cadence_rpm: summary.max_cadence_rpm,
        calories: summary.calories,
    }
}

fn build_gpx_chart_points(
    points: &[TrackPointSample],
    started_at: DateTime<Utc>,
) -> Vec<ActivityChartPoint> {
    let mut chart_points = Vec::with_capacity(points.len());
    let mut cumulative_distance_meters = 0.0;

    for (index, point) in points.iter().enumerate() {
        let speed_mps = if index > 0 {
            let previous = &points[index - 1];
            let segment_distance_meters = match (previous.lat, previous.lon, point.lat, point.lon) {
                (Some(prev_lat), Some(prev_lon), Some(curr_lat), Some(curr_lon)) => {
                    haversine_distance_meters(prev_lat, prev_lon, curr_lat, curr_lon)
                }
                _ => 0.0,
            };
            cumulative_distance_meters += segment_distance_meters;

            match (previous.time, point.time) {
                (Some(start), Some(end)) => seconds_between(start, end)
                    .map(|seconds| segment_distance_meters / f64::from(seconds)),
                _ => None,
            }
        } else {
            None
        };

        chart_points.push(ActivityChartPoint {
            elapsed_seconds: point
                .time
                .and_then(|time| elapsed_seconds_from(started_at, time))
                .unwrap_or(index as i32),
            distance_meters: Some(cumulative_distance_meters),
            elevation_meters: sample_metric_from_f64(point.elevation_meters),
            speed_mps: sample_metric_from_f64(speed_mps),
            heart_rate_bpm: point.heart_rate_bpm,
            cadence_rpm: point.cadence_rpm,
        });
    }

    chart_points
}

fn build_gpx_route_points(
    points: &[TrackPointSample],
    started_at: DateTime<Utc>,
) -> Vec<ActivityRoutePoint> {
    let mut route_points = Vec::with_capacity(points.len());
    let mut cumulative_distance_meters = 0.0;

    for (index, point) in points.iter().enumerate() {
        let (latitude, longitude) = match (point.lat, point.lon) {
            (Some(latitude), Some(longitude)) => (latitude, longitude),
            _ => continue,
        };

        let speed_mps = if index > 0 {
            let previous = &points[index - 1];
            let segment_distance_meters = match (previous.lat, previous.lon, point.lat, point.lon) {
                (Some(prev_lat), Some(prev_lon), Some(curr_lat), Some(curr_lon)) => {
                    haversine_distance_meters(prev_lat, prev_lon, curr_lat, curr_lon)
                }
                _ => 0.0,
            };
            cumulative_distance_meters += segment_distance_meters;

            match (previous.time, point.time) {
                (Some(start), Some(end)) => seconds_between(start, end)
                    .map(|seconds| segment_distance_meters / f64::from(seconds)),
                _ => None,
            }
        } else {
            None
        };

        route_points.push(ActivityRoutePoint {
            elapsed_seconds: point
                .time
                .and_then(|time| elapsed_seconds_from(started_at, time))
                .unwrap_or(index as i32),
            latitude,
            longitude,
            distance_meters: Some(cumulative_distance_meters),
            elevation_meters: sample_metric_from_f64(point.elevation_meters),
            speed_mps: sample_metric_from_f64(speed_mps),
            heart_rate_bpm: point.heart_rate_bpm,
            cadence_rpm: point.cadence_rpm,
        });
    }

    route_points
}

fn build_tcx_chart_points(
    points: &[TrackPointSample],
    started_at: DateTime<Utc>,
) -> Vec<ActivityChartPoint> {
    let mut chart_points = Vec::with_capacity(points.len());
    let mut previous_distance_meters = None;
    let mut previous_time = None;
    let mut last_known_distance_meters = None;

    for (index, point) in points.iter().enumerate() {
        let distance_meters = point.distance_meters.or(last_known_distance_meters);
        let speed_mps = match (
            previous_distance_meters,
            distance_meters,
            previous_time,
            point.time,
        ) {
            (Some(prev_distance), Some(curr_distance), Some(start), Some(end))
                if curr_distance >= prev_distance =>
            {
                seconds_between(start, end)
                    .map(|seconds| (curr_distance - prev_distance) / f64::from(seconds))
            }
            _ => None,
        };

        chart_points.push(ActivityChartPoint {
            elapsed_seconds: point
                .time
                .and_then(|time| elapsed_seconds_from(started_at, time))
                .unwrap_or(index as i32),
            distance_meters: distance_meters
                .and_then(|distance| sample_metric_from_f64(Some(distance))),
            elevation_meters: sample_metric_from_f64(point.elevation_meters),
            speed_mps: sample_metric_from_f64(speed_mps),
            heart_rate_bpm: point.heart_rate_bpm,
            cadence_rpm: point.cadence_rpm,
        });

        previous_distance_meters = distance_meters;
        previous_time = point.time;
        if point.distance_meters.is_some() {
            last_known_distance_meters = point.distance_meters;
        }
    }

    chart_points
}

fn build_tcx_route_points(
    points: &[TrackPointSample],
    started_at: DateTime<Utc>,
) -> Vec<ActivityRoutePoint> {
    let mut route_points = Vec::new();
    let mut previous_distance_meters = None;
    let mut previous_time = None;
    let mut last_known_distance_meters = None;

    for (index, point) in points.iter().enumerate() {
        let (latitude, longitude) = match (point.lat, point.lon) {
            (Some(latitude), Some(longitude)) => (latitude, longitude),
            _ => {
                previous_distance_meters = point.distance_meters.or(previous_distance_meters);
                previous_time = point.time.or(previous_time);
                if point.distance_meters.is_some() {
                    last_known_distance_meters = point.distance_meters;
                }
                continue;
            }
        };

        let distance_meters = point.distance_meters.or(last_known_distance_meters);
        let speed_mps = match (
            previous_distance_meters,
            distance_meters,
            previous_time,
            point.time,
        ) {
            (Some(prev_distance), Some(curr_distance), Some(start), Some(end))
                if curr_distance >= prev_distance =>
            {
                seconds_between(start, end)
                    .map(|seconds| (curr_distance - prev_distance) / f64::from(seconds))
            }
            _ => None,
        };

        route_points.push(ActivityRoutePoint {
            elapsed_seconds: point
                .time
                .and_then(|time| elapsed_seconds_from(started_at, time))
                .unwrap_or(index as i32),
            latitude,
            longitude,
            distance_meters: distance_meters
                .and_then(|distance| sample_metric_from_f64(Some(distance))),
            elevation_meters: sample_metric_from_f64(point.elevation_meters),
            speed_mps: sample_metric_from_f64(speed_mps),
            heart_rate_bpm: point.heart_rate_bpm,
            cadence_rpm: point.cadence_rpm,
        });

        previous_distance_meters = distance_meters;
        previous_time = point.time;
        if point.distance_meters.is_some() {
            last_known_distance_meters = point.distance_meters;
        }
    }

    route_points
}

fn downsample_chart_points(
    chart_points: Vec<ActivityChartPoint>,
    max_points: usize,
) -> Vec<ActivityChartPoint> {
    if chart_points.len() <= max_points || max_points == 0 {
        return chart_points;
    }

    let step = (chart_points.len() - 1) as f64 / (max_points - 1) as f64;
    let mut sampled = Vec::with_capacity(max_points);

    for sample_index in 0..max_points {
        let source_index = ((sample_index as f64) * step).round() as usize;
        if let Some(point) = chart_points.get(source_index).cloned() {
            if sampled.last() != Some(&point) {
                sampled.push(point);
            }
        }
    }

    if let Some(last) = chart_points.last().cloned() {
        if sampled.last() != Some(&last) {
            sampled.push(last);
        }
    }

    sampled
}

fn parse_xml_document<'a>(bytes: &'a [u8], format_name: &str) -> Result<Document<'a>, String> {
    let xml = std::str::from_utf8(bytes)
        .map_err(|_| format!("{format_name} uploads must be UTF-8 XML files"))?;
    Document::parse(xml).map_err(|error| format!("Failed to parse {format_name} file: {error}"))
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
        lat: child_element(node, "Position")
            .and_then(|position| child_text(position, "LatitudeDegrees"))
            .and_then(parse_f64),
        lon: child_element(node, "Position")
            .and_then(|position| child_text(position, "LongitudeDegrees"))
            .and_then(parse_f64),
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
                if curr_distance >= prev_distance {
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

fn sample_metric_from_f64(value: Option<f64>) -> Option<f64> {
    value.filter(|metric| metric.is_finite())
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

fn elapsed_seconds_from(start: DateTime<Utc>, end: DateTime<Utc>) -> Option<i32> {
    let seconds = (end - start).num_seconds();
    if seconds >= 0 && seconds <= i64::from(i32::MAX) {
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
    fn derives_gpx_lap_and_chart_points() {
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

        let detail = derive_activity_detail_data("evening-ride.gpx", "gpx", gpx.as_bytes())
            .expect("gpx detail");

        assert_eq!(detail.laps.len(), 1);
        assert_eq!(detail.laps[0].title, "Full activity");
        assert_eq!(detail.chart_points.len(), 3);
        assert_eq!(detail.route_points.len(), 3);
        assert_eq!(detail.route_points[1].latitude, 35.0005);
        assert_eq!(detail.chart_points[1].heart_rate_bpm, Some(140));
    }

    #[test]
    fn derives_tcx_laps_and_chart_points() {
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
                                                <Position>
                                                    <LatitudeDegrees>35.0000</LatitudeDegrees>
                                                    <LongitudeDegrees>-120.0000</LongitudeDegrees>
                                                </Position>
                        <AltitudeMeters>100</AltitudeMeters>
                        <DistanceMeters>0</DistanceMeters>
                        <HeartRateBpm><Value>130</Value></HeartRateBpm>
                        <Cadence>82</Cadence>
                      </Trackpoint>
                      <Trackpoint>
                        <Time>2026-05-01T12:05:00Z</Time>
                                                <Position>
                                                    <LatitudeDegrees>35.0005</LatitudeDegrees>
                                                    <LongitudeDegrees>-120.0005</LongitudeDegrees>
                                                </Position>
                        <AltitudeMeters>118</AltitudeMeters>
                        <DistanceMeters>2500</DistanceMeters>
                        <HeartRateBpm><Value>140</Value></HeartRateBpm>
                        <Cadence>88</Cadence>
                      </Trackpoint>
                      <Trackpoint>
                        <Time>2026-05-01T12:10:00Z</Time>
                                                <Position>
                                                    <LatitudeDegrees>35.0010</LatitudeDegrees>
                                                    <LongitudeDegrees>-120.0010</LongitudeDegrees>
                                                </Position>
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

        let detail = derive_activity_detail_data("lunch-ride.tcx", "tcx", tcx.as_bytes())
            .expect("tcx detail");

        assert_eq!(detail.laps.len(), 1);
        assert_eq!(detail.laps[0].calories, Some(220));
        assert_eq!(detail.laps[0].max_heart_rate_bpm, Some(158));
        assert_eq!(detail.chart_points.len(), 3);
        assert_eq!(detail.route_points.len(), 3);
        assert_eq!(detail.route_points[2].longitude, -120.0010);
        assert_eq!(detail.chart_points[2].distance_meters, Some(5000.0));
    }

    #[test]
    fn fit_uploads_have_no_detail_data_yet() {
        let detail =
            derive_activity_detail_data("trainer.fit", "fit", b"fit-binary").expect("fit detail");

        assert!(detail.laps.is_empty());
        assert!(detail.chart_points.is_empty());
        assert!(detail.route_points.is_empty());
    }
}
