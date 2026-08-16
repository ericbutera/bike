use crate::activity_details::{
    ActivityChartPoint, ActivityDerivedData, ActivityLap, ActivityRoutePoint,
};
use crate::activity_parser::ParsedActivityData;
use crate::activity_summary::ActivityDraft;
use crate::app_error::AppError;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

const STRAVA_PROVIDER_PAYLOAD_VERSION: u8 = 1;
const MAX_STRAVA_CHART_POINTS: usize = 180;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StoredStravaProviderPayload {
    #[serde(default = "default_payload_version")]
    pub v: u8,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub provider_activity_id: Option<i64>,
    pub activity: StravaActivitySummary,
    #[serde(default)]
    pub streams: StravaActivityStreams,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StravaActivitySummary {
    pub id: i64,
    pub name: String,
    pub distance: Option<f64>,
    pub moving_time: Option<i32>,
    pub elapsed_time: Option<i32>,
    pub max_speed: Option<f64>,
    pub average_heartrate: Option<f64>,
    pub max_heartrate: Option<f64>,
    pub average_cadence: Option<f64>,
    pub calories: Option<f64>,
    pub sport_type: Option<String>,
    #[serde(rename = "type")]
    pub legacy_type: Option<String>,
    pub start_date: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct StravaActivityStreams {
    pub time: Option<StravaStream<i32>>,
    pub distance: Option<StravaStream<f64>>,
    pub latlng: Option<StravaLatLngStream>,
    pub altitude: Option<StravaStream<f64>>,
    pub velocity_smooth: Option<StravaStream<f64>>,
    pub heartrate: Option<StravaStream<i32>>,
    pub cadence: Option<StravaStream<f64>>,
    pub watts: Option<StravaStream<i32>>,
    pub temp: Option<StravaStream<i32>>,
    pub moving: Option<StravaStream<bool>>,
    pub grade_smooth: Option<StravaStream<f64>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StravaStream<T> {
    pub data: Vec<T>,
    pub original_size: Option<i32>,
    pub resolution: Option<String>,
    pub series_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StravaLatLngStream {
    pub data: Vec<[f64; 2]>,
    pub original_size: Option<i32>,
    pub resolution: Option<String>,
    pub series_type: Option<String>,
}

impl StoredStravaProviderPayload {
    pub fn new(activity: StravaActivitySummary, streams: StravaActivityStreams) -> Self {
        Self {
            v: STRAVA_PROVIDER_PAYLOAD_VERSION,
            provider: default_provider(),
            provider_activity_id: Some(activity.id),
            activity,
            streams,
        }
    }
}

pub fn parse_strava_provider_payload(bytes: &[u8]) -> Result<ParsedActivityData, AppError> {
    let payload: StoredStravaProviderPayload = serde_json::from_slice(bytes).map_err(|error| {
        AppError::validation_field(
            "file",
            format!("Failed to parse Strava provider payload: {error}"),
        )
    })?;
    if payload.provider != "strava" {
        return Err(AppError::validation_field(
            "file",
            format!("Unsupported provider payload: {}", payload.provider),
        ));
    }
    if payload.v != STRAVA_PROVIDER_PAYLOAD_VERSION {
        return Err(AppError::validation_field(
            "file",
            format!("Unsupported Strava provider payload version: {}", payload.v),
        ));
    }

    let draft = build_strava_activity_draft(&payload.activity, &payload.streams);
    let derived_data = build_strava_derived_data(&draft, &payload.streams);
    Ok(ParsedActivityData {
        draft,
        derived_data,
    })
}

fn build_strava_activity_draft(
    activity: &StravaActivitySummary,
    streams: &StravaActivityStreams,
) -> ActivityDraft {
    let total_time_seconds = positive_i32(activity.elapsed_time.or(activity.moving_time));
    let moving_time_seconds = positive_i32(activity.moving_time).or(total_time_seconds);
    let ended_at = total_time_seconds
        .map(|seconds| activity.start_date + Duration::seconds(i64::from(seconds)));
    let distance_meters = positive_f64(activity.distance).or_else(|| {
        streams
            .distance
            .as_ref()
            .and_then(|stream| stream.data.last().copied())
    });
    let average_speed_mps = match (distance_meters, moving_time_seconds.or(total_time_seconds)) {
        (Some(distance), Some(seconds)) if seconds > 0 => Some(distance / f64::from(seconds)),
        _ => None,
    };
    let heart_rates = streams
        .heartrate
        .as_ref()
        .map(|stream| stream.data.as_slice())
        .unwrap_or(&[]);
    let cadences = streams
        .cadence
        .as_ref()
        .map(|stream| stream.data.as_slice())
        .unwrap_or(&[]);
    let elevations = streams
        .altitude
        .as_ref()
        .map(|stream| stream.data.as_slice())
        .unwrap_or(&[]);
    let (elevation_gain_meters, elevation_loss_meters) = summarize_elevation(elevations);

    ActivityDraft {
        title: activity.name.clone(),
        sport: normalize_strava_sport(activity),
        started_at: activity.start_date,
        ended_at,
        distance_meters: positive_f64(distance_meters),
        moving_time_seconds,
        total_time_seconds,
        elevation_gain_meters,
        elevation_loss_meters,
        average_speed_mps: positive_f64(activity.average_speed_mps().or(average_speed_mps)),
        max_speed_mps: positive_f64(
            activity
                .max_speed
                .or_else(|| max_f64_stream(&streams.velocity_smooth)),
        ),
        average_heart_rate_bpm: activity
            .average_heartrate
            .map(round_i32)
            .or_else(|| average_i32(heart_rates)),
        max_heart_rate_bpm: activity
            .max_heartrate
            .map(round_i32)
            .or_else(|| max_i32(heart_rates)),
        average_cadence_rpm: activity
            .average_cadence
            .map(round_i32)
            .or_else(|| average_f64(cadences).map(round_i32)),
        max_cadence_rpm: max_f64(cadences).map(round_i32),
        calories: activity.calories.map(round_i32),
    }
}

fn build_strava_derived_data(
    draft: &ActivityDraft,
    streams: &StravaActivityStreams,
) -> ActivityDerivedData {
    let chart_points =
        downsample_chart_points(build_strava_chart_points(streams), MAX_STRAVA_CHART_POINTS);
    ActivityDerivedData {
        laps: vec![full_activity_lap(draft)],
        route_points: build_strava_route_points(streams),
        chart_points,
    }
}

fn build_strava_chart_points(streams: &StravaActivityStreams) -> Vec<ActivityChartPoint> {
    (0..max_stream_count(streams))
        .map(|index| ActivityChartPoint {
            elapsed_seconds: stream_i32_value(streams.time.as_ref(), index).unwrap_or(index as i32),
            distance_meters: stream_f64_value(streams.distance.as_ref(), index),
            elevation_meters: stream_f64_value(streams.altitude.as_ref(), index),
            speed_mps: stream_f64_value(streams.velocity_smooth.as_ref(), index),
            heart_rate_bpm: stream_i32_value(streams.heartrate.as_ref(), index),
            cadence_rpm: stream_f64_value(streams.cadence.as_ref(), index).map(round_i32),
            power_watts: stream_i32_value(streams.watts.as_ref(), index),
        })
        .collect()
}

fn build_strava_route_points(streams: &StravaActivityStreams) -> Vec<ActivityRoutePoint> {
    (0..max_stream_count(streams))
        .filter_map(|index| {
            let [latitude, longitude] = stream_latlng_value(streams, index)?;
            Some(ActivityRoutePoint {
                elapsed_seconds: stream_i32_value(streams.time.as_ref(), index)
                    .unwrap_or(index as i32),
                latitude,
                longitude,
                distance_meters: stream_f64_value(streams.distance.as_ref(), index),
                elevation_meters: stream_f64_value(streams.altitude.as_ref(), index),
                speed_mps: stream_f64_value(streams.velocity_smooth.as_ref(), index),
                heart_rate_bpm: stream_i32_value(streams.heartrate.as_ref(), index),
                cadence_rpm: stream_f64_value(streams.cadence.as_ref(), index).map(round_i32),
                power_watts: stream_i32_value(streams.watts.as_ref(), index),
            })
        })
        .collect()
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

fn max_stream_count(streams: &StravaActivityStreams) -> usize {
    [
        streams
            .time
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .distance
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .latlng
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .altitude
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .velocity_smooth
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .heartrate
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .cadence
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
        streams
            .watts
            .as_ref()
            .map(|stream| stream.data.len())
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

fn downsample_chart_points(
    points: Vec<ActivityChartPoint>,
    max_points: usize,
) -> Vec<ActivityChartPoint> {
    if points.len() <= max_points || max_points == 0 {
        return points;
    }

    let last_index = points.len() - 1;
    (0..max_points)
        .map(|index| {
            let source_index =
                ((index as f64) * (last_index as f64) / ((max_points - 1) as f64)).round() as usize;
            points[source_index].clone()
        })
        .collect()
}

fn normalize_strava_sport(activity: &StravaActivitySummary) -> String {
    match activity
        .sport_type
        .as_deref()
        .or(activity.legacy_type.as_deref())
        .unwrap_or("activity")
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "ride" | "virtualride" | "mountainbikeride" | "gravelride" | "ebikeride"
        | "emountainbikeride" | "velomobile" | "handcycle" => "ride".to_string(),
        "run" | "virtualrun" | "trailrun" => "run".to_string(),
        "swim" => "swim".to_string(),
        "" => "activity".to_string(),
        other => other.to_string(),
    }
}

fn summarize_elevation(values: &[f64]) -> (Option<f64>, Option<f64>) {
    let mut gain = 0.0;
    let mut loss = 0.0;
    for window in values.windows(2) {
        let delta = window[1] - window[0];
        if delta > 0.0 {
            gain += delta;
        } else {
            loss += delta.abs();
        }
    }

    (positive_f64(Some(gain)), positive_f64(Some(loss)))
}

fn stream_i32_value(stream: Option<&StravaStream<i32>>, index: usize) -> Option<i32> {
    stream.and_then(|stream| stream.data.get(index)).copied()
}

fn stream_f64_value(stream: Option<&StravaStream<f64>>, index: usize) -> Option<f64> {
    stream.and_then(|stream| stream.data.get(index)).copied()
}

fn stream_latlng_value(streams: &StravaActivityStreams, index: usize) -> Option<[f64; 2]> {
    streams.latlng.as_ref()?.data.get(index).copied()
}

fn average_i32(values: &[i32]) -> Option<i32> {
    if values.is_empty() {
        None
    } else {
        Some(round_i32(
            values.iter().copied().map(f64::from).sum::<f64>() / values.len() as f64,
        ))
    }
}

fn average_f64(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().copied().sum::<f64>() / values.len() as f64)
    }
}

fn max_i32(values: &[i32]) -> Option<i32> {
    values.iter().copied().max()
}

fn max_f64(values: &[f64]) -> Option<f64> {
    values.iter().copied().reduce(f64::max)
}

fn max_f64_stream(stream: &Option<StravaStream<f64>>) -> Option<f64> {
    max_f64(stream.as_ref()?.data.as_slice())
}

fn positive_i32(value: Option<i32>) -> Option<i32> {
    value.filter(|value| *value > 0)
}

fn positive_f64(value: Option<f64>) -> Option<f64> {
    value
        .filter(|value| value.is_finite())
        .filter(|value| *value > 0.0)
}

fn round_i32(value: f64) -> i32 {
    value.round() as i32
}

fn default_payload_version() -> u8 {
    STRAVA_PROVIDER_PAYLOAD_VERSION
}

fn default_provider() -> String {
    "strava".to_string()
}

trait StravaActivitySummaryExt {
    fn average_speed_mps(&self) -> Option<f64>;
}

impl StravaActivitySummaryExt for StravaActivitySummary {
    fn average_speed_mps(&self) -> Option<f64> {
        match (self.distance, self.moving_time.or(self.elapsed_time)) {
            (Some(distance), Some(seconds)) if seconds > 0 => Some(distance / f64::from(seconds)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_strava_provider_payload_directly() {
        let payload = StoredStravaProviderPayload::new(
            StravaActivitySummary {
                id: 99,
                name: "Lunch Ride".to_string(),
                distance: Some(1000.0),
                moving_time: Some(300),
                elapsed_time: Some(320),
                max_speed: Some(6.2),
                average_heartrate: Some(145.0),
                max_heartrate: Some(162.0),
                average_cadence: Some(88.0),
                calories: Some(120.0),
                sport_type: Some("Ride".to_string()),
                legacy_type: Some("Ride".to_string()),
                start_date: DateTime::parse_from_rfc3339("2026-05-12T12:00:00Z")
                    .unwrap()
                    .with_timezone(&Utc),
            },
            StravaActivityStreams {
                time: Some(test_stream(vec![0, 160, 320])),
                distance: Some(test_stream(vec![0.0, 500.0, 1000.0])),
                latlng: Some(StravaLatLngStream {
                    data: vec![[35.0, -82.0], [35.0005, -82.0005], [35.001, -82.001]],
                    original_size: Some(3),
                    resolution: Some("high".to_string()),
                    series_type: Some("distance".to_string()),
                }),
                altitude: Some(test_stream(vec![700.0, 720.0, 725.0])),
                velocity_smooth: Some(test_stream(vec![3.0, 4.0, 5.0])),
                heartrate: Some(test_stream(vec![140, 145, 150])),
                cadence: Some(test_stream(vec![86.0, 88.0, 90.0])),
                watts: Some(test_stream(vec![205, 220, 235])),
                temp: None,
                moving: None,
                grade_smooth: None,
            },
        );

        let parsed = parse_strava_provider_payload(&serde_json::to_vec(&payload).unwrap()).unwrap();

        assert_eq!(parsed.draft.title, "Lunch Ride");
        assert_eq!(parsed.draft.sport, "ride");
        assert_eq!(parsed.draft.distance_meters, Some(1000.0));
        assert_eq!(parsed.derived_data.route_points.len(), 3);
        assert_eq!(parsed.derived_data.route_points[0].power_watts, Some(205));
        assert_eq!(parsed.derived_data.chart_points[1].cadence_rpm, Some(88));
        assert_eq!(parsed.derived_data.laps[0].duration_seconds, Some(320));
    }

    fn test_stream<T>(data: Vec<T>) -> StravaStream<T> {
        StravaStream {
            data,
            original_size: None,
            resolution: None,
            series_type: None,
        }
    }
}
