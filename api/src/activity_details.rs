use crate::activity_summary::{summarize_activity_upload, ActivityDraft};
use crate::app_error::AppError;
use crate::fit_support::parse_fit_activity;
use chrono::{DateTime, Utc};
use roxmltree::{Document, Node};
use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

const MAX_CHART_POINTS: usize = 180;
const STORAGE_FORMAT_VERSION: u8 = 2;
const STORAGE_COORDINATE_SCALE: f64 = 10_000_000.0;
const STORAGE_DISTANCE_SCALE: f64 = 10.0;
const STORAGE_ELEVATION_SCALE: f64 = 10.0;
const STORAGE_SPEED_SCALE: f64 = 100.0;

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
    pub power_watts: Option<i32>,
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
    pub power_watts: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, FromJsonQueryResult)]
pub struct StoredActivityDerivedData {
    v: u8,
    laps: Vec<ActivityLap>,
    chart_points: Vec<ActivityChartPoint>,
    route_points: Vec<ActivityRoutePoint>,
}

impl Default for StoredActivityDerivedData {
    fn default() -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: Vec::new(),
            chart_points: Vec::new(),
            route_points: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredActivityDerivedDataV2 {
    v: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityDerivedDataV1 {
    v: u8,
    #[serde(default, rename = "l")]
    laps: Vec<CompactStoredActivityLapV1>,
    #[serde(default, rename = "c")]
    chart_points: Vec<CompactStoredActivityChartPointV1>,
    #[serde(default, rename = "r")]
    route_points: Vec<CompactStoredActivityRoutePointV1>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityLapV1(
    i32,
    String,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityChartPointV1(
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityRoutePointV1(
    i32,
    i32,
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, Default, PartialEq, FromJsonQueryResult)]
pub struct StoredRoutePointSeries(Vec<ActivityRoutePoint>);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum StoredRoutePointSeriesRepr {
    V2(Vec<ActivityRoutePoint>),
    V1(Vec<CompactStoredActivityRoutePointV1>),
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
    power_watts: Option<i32>,
}

impl Serialize for StoredActivityDerivedData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        StoredActivityDerivedDataV2 {
            v: STORAGE_FORMAT_VERSION,
            laps: self.laps.clone(),
            chart_points: self.chart_points.clone(),
            route_points: self.route_points.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StoredActivityDerivedData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let is_schemaful = value.get("laps").is_some()
            || value.get("chart_points").is_some()
            || value.get("route_points").is_some();

        if is_schemaful {
            let value = StoredActivityDerivedDataV2::deserialize(value)
                .map_err(serde::de::Error::custom)?;
            if value.v != STORAGE_FORMAT_VERSION {
                tracing::warn!(
                    version = value.v,
                    "unsupported activity derived data format version"
                );
                return Ok(Self::default());
            }

            Ok(Self {
                v: STORAGE_FORMAT_VERSION,
                laps: value.laps,
                chart_points: value.chart_points,
                route_points: value.route_points,
            })
        } else {
            let value = CompactStoredActivityDerivedDataV1::deserialize(value)
                .map_err(serde::de::Error::custom)?;
            if value.v != 1 {
                tracing::warn!(
                    version = value.v,
                    "unsupported activity derived data format version"
                );
                return Ok(Self::default());
            }

            Ok(Self::from(value))
        }
    }
}

impl Serialize for StoredRoutePointSeries {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StoredRoutePointSeries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match StoredRoutePointSeriesRepr::deserialize(deserializer)? {
            StoredRoutePointSeriesRepr::V2(value) => Ok(Self(value)),
            StoredRoutePointSeriesRepr::V1(value) => Ok(Self(
                value.into_iter().map(ActivityRoutePoint::from).collect(),
            )),
        }
    }
}

pub fn derive_activity_detail_data(
    filename: &str,
    format: &str,
    bytes: &[u8],
) -> Result<ActivityDerivedData, AppError> {
    let result = match format {
        "gpx" => derive_gpx_activity_detail(filename, bytes),
        "tcx" => derive_tcx_activity_detail(filename, bytes),
        "fit" => derive_fit_activity_detail(filename, bytes),
        _ => Err("Only .fit, .tcx, and .gpx uploads are supported".to_string()),
    };

    result.map_err(|message| AppError::validation_field("file", message))
}

pub fn serialize_derived_activity_data(
    data: &ActivityDerivedData,
) -> Result<StoredActivityDerivedData, AppError> {
    Ok(StoredActivityDerivedData::from(data))
}

pub fn deserialize_derived_activity_data(
    raw: Option<&StoredActivityDerivedData>,
) -> ActivityDerivedData {
    match raw {
        Some(value) => ActivityDerivedData::from(value.clone()),
        _ => ActivityDerivedData::default(),
    }
}

pub fn serialize_route_point_series(
    route_points: &[ActivityRoutePoint],
) -> Result<StoredRoutePointSeries, AppError> {
    Ok(StoredRoutePointSeries(route_points.to_vec()))
}

pub fn deserialize_route_point_series(
    raw: Option<&StoredRoutePointSeries>,
) -> Vec<ActivityRoutePoint> {
    raw.map(|series| series.0.clone()).unwrap_or_default()
}

impl From<&ActivityDerivedData> for StoredActivityDerivedData {
    fn from(value: &ActivityDerivedData) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: value.laps.clone(),
            chart_points: value.chart_points.clone(),
            route_points: value.route_points.clone(),
        }
    }
}

impl From<StoredActivityDerivedData> for ActivityDerivedData {
    fn from(value: StoredActivityDerivedData) -> Self {
        Self {
            laps: value.laps,
            chart_points: value.chart_points,
            route_points: value.route_points,
        }
    }
}

impl From<CompactStoredActivityDerivedDataV1> for StoredActivityDerivedData {
    fn from(value: CompactStoredActivityDerivedDataV1) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: value.laps.into_iter().map(ActivityLap::from).collect(),
            chart_points: value
                .chart_points
                .into_iter()
                .map(ActivityChartPoint::from)
                .collect(),
            route_points: value
                .route_points
                .into_iter()
                .map(ActivityRoutePoint::from)
                .collect(),
        }
    }
}

impl From<CompactStoredActivityLapV1> for ActivityLap {
    fn from(value: CompactStoredActivityLapV1) -> Self {
        Self {
            lap_index: value.0,
            title: value.1,
            start_offset_seconds: value.2,
            duration_seconds: value.3,
            distance_meters: decode_scaled_metric(value.4, STORAGE_DISTANCE_SCALE),
            elevation_gain_meters: decode_scaled_metric(value.5, STORAGE_ELEVATION_SCALE),
            elevation_loss_meters: decode_scaled_metric(value.6, STORAGE_ELEVATION_SCALE),
            average_speed_mps: decode_scaled_metric(value.7, STORAGE_SPEED_SCALE),
            max_speed_mps: decode_scaled_metric(value.8, STORAGE_SPEED_SCALE),
            average_heart_rate_bpm: value.9,
            max_heart_rate_bpm: value.10,
            average_cadence_rpm: value.11,
            max_cadence_rpm: value.12,
            calories: value.13,
        }
    }
}

impl From<CompactStoredActivityChartPointV1> for ActivityChartPoint {
    fn from(value: CompactStoredActivityChartPointV1) -> Self {
        Self {
            elapsed_seconds: value.0,
            distance_meters: decode_scaled_metric(value.1, STORAGE_DISTANCE_SCALE),
            elevation_meters: decode_scaled_metric(value.2, STORAGE_ELEVATION_SCALE),
            speed_mps: decode_scaled_metric(value.3, STORAGE_SPEED_SCALE),
            heart_rate_bpm: value.4,
            cadence_rpm: value.5,
            power_watts: None,
        }
    }
}

impl From<CompactStoredActivityRoutePointV1> for ActivityRoutePoint {
    fn from(value: CompactStoredActivityRoutePointV1) -> Self {
        Self {
            elapsed_seconds: value.0,
            latitude: decode_coordinate(value.1),
            longitude: decode_coordinate(value.2),
            distance_meters: decode_scaled_metric(value.3, STORAGE_DISTANCE_SCALE),
            elevation_meters: decode_scaled_metric(value.4, STORAGE_ELEVATION_SCALE),
            speed_mps: decode_scaled_metric(value.5, STORAGE_SPEED_SCALE),
            heart_rate_bpm: value.6,
            cadence_rpm: value.7,
            power_watts: None,
        }
    }
}

fn decode_coordinate(value: i32) -> f64 {
    f64::from(value) / STORAGE_COORDINATE_SCALE
}

fn decode_scaled_metric(value: Option<i32>, scale: f64) -> Option<f64> {
    value.map(|metric| f64::from(metric) / scale)
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

fn derive_fit_activity_detail(filename: &str, bytes: &[u8]) -> Result<ActivityDerivedData, String> {
    let parsed = parse_fit_activity(filename, bytes)?;
    let chart_points = downsample_chart_points(
        parsed
            .track_points
            .iter()
            .map(|point| ActivityChartPoint {
                elapsed_seconds: elapsed_seconds_from(parsed.draft.started_at, point.timestamp)
                    .unwrap_or(0),
                distance_meters: point.distance_meters,
                elevation_meters: point.elevation_meters,
                speed_mps: point.speed_mps,
                heart_rate_bpm: point.heart_rate_bpm,
                cadence_rpm: point.cadence_rpm,
                power_watts: point.power_watts,
            })
            .collect(),
        MAX_CHART_POINTS,
    );
    let route_points = parsed
        .track_points
        .iter()
        .filter_map(|point| {
            Some(ActivityRoutePoint {
                elapsed_seconds: elapsed_seconds_from(parsed.draft.started_at, point.timestamp)
                    .unwrap_or(0),
                latitude: point.latitude?,
                longitude: point.longitude?,
                distance_meters: point.distance_meters,
                elevation_meters: point.elevation_meters,
                speed_mps: point.speed_mps,
                heart_rate_bpm: point.heart_rate_bpm,
                cadence_rpm: point.cadence_rpm,
                power_watts: point.power_watts,
            })
        })
        .collect::<Vec<_>>();

    let laps = if parsed.laps.is_empty() {
        vec![full_activity_lap(&parsed.draft)]
    } else {
        let mut fallback_start_offset_seconds = 0;

        parsed
            .laps
            .iter()
            .enumerate()
            .map(|(index, lap)| {
                let start_offset_seconds = lap
                    .start_time
                    .and_then(|start| elapsed_seconds_from(parsed.draft.started_at, start))
                    .or(Some(fallback_start_offset_seconds));

                if let Some(duration_seconds) = lap.duration_seconds {
                    fallback_start_offset_seconds += duration_seconds;
                }

                ActivityLap {
                    lap_index: (index + 1) as i32,
                    title: format!("Lap {}", index + 1),
                    start_offset_seconds,
                    duration_seconds: lap.duration_seconds,
                    distance_meters: lap.distance_meters,
                    elevation_gain_meters: lap.elevation_gain_meters,
                    elevation_loss_meters: lap.elevation_loss_meters,
                    average_speed_mps: lap.average_speed_mps,
                    max_speed_mps: lap.max_speed_mps,
                    average_heart_rate_bpm: lap.average_heart_rate_bpm,
                    max_heart_rate_bpm: lap.max_heart_rate_bpm,
                    average_cadence_rpm: lap.average_cadence_rpm,
                    max_cadence_rpm: lap.max_cadence_rpm,
                    calories: lap.calories,
                }
            })
            .collect()
    };

    Ok(ActivityDerivedData {
        laps,
        chart_points,
        route_points,
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
            power_watts: point.power_watts,
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
            power_watts: point.power_watts,
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
            power_watts: point.power_watts,
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
            power_watts: point.power_watts,
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
    let xml = xml
        .strip_prefix('\u{feff}')
        .unwrap_or(xml)
        .trim_start_matches(|character: char| character.is_whitespace());
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
        power_watts: descendant_text(node, "power")
            .as_deref()
            .and_then(parse_i32),
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
        power_watts: descendant_text(node, "Watts")
            .as_deref()
            .and_then(parse_i32),
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
    use serde_json::json;

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
    fn derives_tcx_details_with_leading_whitespace_before_xml_declaration() {
        let tcx = "        <?xml version=\"1.0\" encoding=\"utf-8\"?>\n        <TrainingCenterDatabase xmlns=\"http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2\">\n          <Activities>\n            <Activity Sport=\"Biking\">\n              <Id>2026-05-01T10:00:00Z</Id>\n              <Lap StartTime=\"2026-05-01T10:00:00Z\">\n                <TotalTimeSeconds>300</TotalTimeSeconds>\n                <DistanceMeters>1200</DistanceMeters>\n                <Track>\n                  <Trackpoint>\n                    <Time>2026-05-01T10:00:00Z</Time>\n                    <Position>\n                      <LatitudeDegrees>45.0</LatitudeDegrees>\n                      <LongitudeDegrees>-122.0</LongitudeDegrees>\n                    </Position>\n                    <AltitudeMeters>10</AltitudeMeters>\n                    <DistanceMeters>0</DistanceMeters>\n                  </Trackpoint>\n                  <Trackpoint>\n                    <Time>2026-05-01T10:05:00Z</Time>\n                    <Position>\n                      <LatitudeDegrees>45.01</LatitudeDegrees>\n                      <LongitudeDegrees>-122.01</LongitudeDegrees>\n                    </Position>\n                    <AltitudeMeters>15</AltitudeMeters>\n                    <DistanceMeters>1200</DistanceMeters>\n                  </Trackpoint>\n                </Track>\n              </Lap>\n            </Activity>\n          </Activities>\n        </TrainingCenterDatabase>\n";

        let detail = derive_activity_detail_data("whitespace.tcx", "tcx", tcx.as_bytes())
            .expect("tcx detail with leading whitespace");

        assert_eq!(detail.chart_points.len(), 2);
        assert_eq!(detail.route_points.len(), 2);
    }

    #[test]
    fn derives_fit_laps_and_chart_points() {
        let fit = include_bytes!("../tests/fixtures/activity.fit");
        let detail = derive_activity_detail_data("activity.fit", "fit", fit).expect("fit detail");

        assert_eq!(detail.laps.len(), 1);
        assert!(!detail.chart_points.is_empty());
        assert!(!detail.route_points.is_empty());
        assert_eq!(detail.laps[0].distance_meters, Some(5.73));
        assert_eq!(detail.laps[0].duration_seconds, Some(14));
    }

    #[test]
    fn serializes_activity_derived_data_to_compact_shape() {
        let stored = serialize_derived_activity_data(&ActivityDerivedData {
            laps: vec![ActivityLap {
                lap_index: 0,
                title: "Lap 1".to_string(),
                start_offset_seconds: Some(5),
                duration_seconds: Some(90),
                distance_meters: Some(1234.5),
                elevation_gain_meters: Some(12.3),
                elevation_loss_meters: Some(4.5),
                average_speed_mps: Some(6.78),
                max_speed_mps: Some(9.01),
                average_heart_rate_bpm: Some(140),
                max_heart_rate_bpm: Some(171),
                average_cadence_rpm: Some(88),
                max_cadence_rpm: Some(102),
                calories: Some(77),
            }],
            chart_points: vec![ActivityChartPoint {
                elapsed_seconds: 15,
                distance_meters: Some(25.4),
                elevation_meters: Some(8.2),
                speed_mps: Some(7.65),
                heart_rate_bpm: Some(145),
                cadence_rpm: Some(91),
                power_watts: Some(210),
            }],
            route_points: vec![ActivityRoutePoint {
                elapsed_seconds: 15,
                latitude: 45.1234567,
                longitude: -122.7654321,
                distance_meters: Some(25.4),
                elevation_meters: Some(8.2),
                speed_mps: Some(7.65),
                heart_rate_bpm: Some(145),
                cadence_rpm: Some(91),
                power_watts: Some(210),
            }],
        })
        .expect("serialized activity derived data");

        let value = serde_json::to_value(stored).expect("json value");

        assert_eq!(
            value,
            json!({
                "v": 2,
                "laps": [{
                    "lap_index": 0,
                    "title": "Lap 1",
                    "start_offset_seconds": 5,
                    "duration_seconds": 90,
                    "distance_meters": 1234.5,
                    "elevation_gain_meters": 12.3,
                    "elevation_loss_meters": 4.5,
                    "average_speed_mps": 6.78,
                    "max_speed_mps": 9.01,
                    "average_heart_rate_bpm": 140,
                    "max_heart_rate_bpm": 171,
                    "average_cadence_rpm": 88,
                    "max_cadence_rpm": 102,
                    "calories": 77,
                }],
                "chart_points": [{
                    "elapsed_seconds": 15,
                    "distance_meters": 25.4,
                    "elevation_meters": 8.2,
                    "speed_mps": 7.65,
                    "heart_rate_bpm": 145,
                    "cadence_rpm": 91,
                    "power_watts": 210,
                }],
                "route_points": [{
                    "elapsed_seconds": 15,
                    "latitude": 45.1234567,
                    "longitude": -122.7654321,
                    "distance_meters": 25.4,
                    "elevation_meters": 8.2,
                    "speed_mps": 7.65,
                    "heart_rate_bpm": 145,
                    "cadence_rpm": 91,
                    "power_watts": 210,
                }],
            })
        );
    }

    #[test]
    fn deserializes_compact_activity_derived_data() {
        let stored = serde_json::from_value::<StoredActivityDerivedData>(json!({
            "v": 1,
            "l": [[0, "Lap 1", 5, 90, 12345, 123, 45, 678, 901, 140, 171, 88, 102, 77]],
            "c": [[15, 254, 82, 765, 145, 91]],
            "r": [[15, 451234567, -1227654321, 254, 82, 765, 145, 91]],
        }))
        .expect("stored data");
        let derived = deserialize_derived_activity_data(Some(&stored));

        assert_eq!(derived.laps.len(), 1);
        assert_eq!(derived.chart_points.len(), 1);
        assert_eq!(derived.route_points.len(), 1);
        assert_eq!(derived.laps[0].distance_meters, Some(1234.5));
        assert_eq!(derived.route_points[0].latitude, 45.1234567);
        assert_eq!(derived.route_points[0].longitude, -122.7654321);
        assert_eq!(derived.route_points[0].speed_mps, Some(7.65));
        assert_eq!(derived.chart_points[0].power_watts, None);
    }

    #[test]
    fn serializes_route_point_series_to_schemaful_objects() {
        let stored = serialize_route_point_series(&[ActivityRoutePoint {
            elapsed_seconds: 15,
            latitude: 45.1234567,
            longitude: -122.7654321,
            distance_meters: Some(25.4),
            elevation_meters: Some(8.2),
            speed_mps: Some(7.65),
            heart_rate_bpm: Some(145),
            cadence_rpm: Some(91),
            power_watts: Some(210),
        }])
        .expect("stored route series");

        let value = serde_json::to_value(stored).expect("json value");
        assert_eq!(
            value,
            json!([{
                    "elapsed_seconds": 15,
                    "latitude": 45.1234567,
                    "longitude": -122.7654321,
                    "distance_meters": 25.4,
                    "elevation_meters": 8.2,
                    "speed_mps": 7.65,
                    "heart_rate_bpm": 145,
                    "cadence_rpm": 91,
                    "power_watts": 210,
            }])
        );
    }

    #[test]
    fn derives_tcx_trackpoint_power_from_extensions() {
        let tcx = r#"
                        <TrainingCenterDatabase xmlns="http://www.garmin.com/xmlschemas/TrainingCenterDatabase/v2"
                                xmlns:ns3="http://www.garmin.com/xmlschemas/ActivityExtension/v2">
                            <Activities>
                                <Activity Sport="Biking">
                                    <Id>2026-05-01T12:00:00Z</Id>
                                    <Lap StartTime="2026-05-01T12:00:00Z">
                                        <TotalTimeSeconds>600</TotalTimeSeconds>
                                        <DistanceMeters>5000</DistanceMeters>
                                        <Track>
                                            <Trackpoint>
                                                <Time>2026-05-01T12:00:00Z</Time>
                                                <DistanceMeters>0</DistanceMeters>
                                                <Extensions>
                                                    <ns3:TPX>
                                                        <ns3:Watts>205</ns3:Watts>
                                                    </ns3:TPX>
                                                </Extensions>
                                            </Trackpoint>
                                        </Track>
                                    </Lap>
                                </Activity>
                            </Activities>
                        </TrainingCenterDatabase>
                "#;

        let detail = derive_activity_detail_data("power.tcx", "tcx", tcx.as_bytes())
            .expect("tcx detail with power");

        assert_eq!(detail.chart_points[0].power_watts, Some(205));
    }

    #[test]
    fn deserializes_compact_route_point_series() {
        let stored = serde_json::from_value::<StoredRoutePointSeries>(json!([[
            15,
            451234567,
            -1227654321,
            254,
            82,
            765,
            145,
            91
        ]]))
        .expect("stored route series");

        let route_points = deserialize_route_point_series(Some(&stored));

        assert_eq!(route_points.len(), 1);
        assert_eq!(route_points[0].latitude, 45.1234567);
        assert_eq!(route_points[0].power_watts, None);
    }
}
