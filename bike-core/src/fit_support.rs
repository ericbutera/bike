use crate::activity_summary::ActivityDraft;
use chrono::{DateTime, Utc};
use fitparser::{profile::MesgNum, FitDataRecord, Value};
use std::io::Cursor;
use std::path::Path;

const SEMICIRCLES_TO_DEGREES: f64 = 180.0 / 2_147_483_648.0;

#[derive(Debug, Clone)]
pub struct FitTrackPoint {
    pub timestamp: DateTime<Utc>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub distance_meters: Option<f64>,
    pub elevation_meters: Option<f64>,
    pub speed_mps: Option<f64>,
    pub heart_rate_bpm: Option<i32>,
    pub cadence_rpm: Option<i32>,
    pub power_watts: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct FitLapSummary {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
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

pub struct ParsedFitActivity {
    pub draft: ActivityDraft,
    pub track_points: Vec<FitTrackPoint>,
    pub laps: Vec<FitLapSummary>,
}

pub fn parse_fit_activity(filename: &str, bytes: &[u8]) -> Result<ParsedFitActivity, String> {
    let mut cursor = Cursor::new(bytes);
    let records = fitparser::from_reader(&mut cursor)
        .map_err(|error| format!("Failed to parse FIT file: {error}"))?;

    let track_points = records
        .iter()
        .filter(|record| record.kind() == MesgNum::Record)
        .filter_map(parse_fit_track_point)
        .collect::<Vec<_>>();
    let laps = records
        .iter()
        .filter(|record| record.kind() == MesgNum::Lap)
        .map(parse_fit_lap)
        .collect::<Vec<_>>();
    let session = records
        .iter()
        .rev()
        .find(|record| record.kind() == MesgNum::Session);

    let started_at = field_datetime(session, "start_time")
        .or_else(|| track_points.first().map(|point| point.timestamp))
        .unwrap_or_else(Utc::now);
    let ended_at = field_datetime(session, "timestamp")
        .or_else(|| track_points.last().map(|point| point.timestamp))
        .or_else(|| laps.last().and_then(|lap| lap.end_time));
    let total_time_seconds = field_seconds(session, "total_elapsed_time")
        .or_else(|| ended_at.and_then(|end| seconds_between(started_at, end)));
    let moving_time_seconds = field_seconds(session, "total_timer_time").or(total_time_seconds);

    let heart_rates = track_points
        .iter()
        .filter_map(|point| point.heart_rate_bpm)
        .collect::<Vec<_>>();
    let cadences = track_points
        .iter()
        .filter_map(|point| point.cadence_rpm)
        .collect::<Vec<_>>();
    let max_speed_from_records = track_points
        .iter()
        .filter_map(|point| point.speed_mps)
        .reduce(f64::max);
    let distance_from_records = track_points
        .iter()
        .filter_map(|point| point.distance_meters)
        .reduce(f64::max);

    Ok(ParsedFitActivity {
        draft: ActivityDraft {
            title: humanize_filename(filename),
            sport: normalize_sport(field_string(session, "sport").as_deref()),
            started_at,
            ended_at,
            distance_meters: field_f64(session, "total_distance").or(distance_from_records),
            moving_time_seconds,
            total_time_seconds,
            elevation_gain_meters: field_f64(session, "total_ascent"),
            elevation_loss_meters: field_f64(session, "total_descent"),
            average_speed_mps: field_f64(session, "enhanced_avg_speed")
                .or_else(|| field_f64(session, "avg_speed")),
            max_speed_mps: field_f64(session, "enhanced_max_speed")
                .or_else(|| field_f64(session, "max_speed"))
                .or(max_speed_from_records),
            average_heart_rate_bpm: field_i32(session, "avg_heart_rate")
                .or_else(|| average_metric(&heart_rates)),
            max_heart_rate_bpm: field_i32(session, "max_heart_rate")
                .or_else(|| max_metric(&heart_rates)),
            average_cadence_rpm: field_i32(session, "avg_cadence")
                .or_else(|| average_metric(&cadences)),
            max_cadence_rpm: field_i32(session, "max_cadence").or_else(|| max_metric(&cadences)),
            calories: field_i32(session, "total_calories"),
        },
        track_points,
        laps,
    })
}

fn parse_fit_track_point(record: &FitDataRecord) -> Option<FitTrackPoint> {
    let timestamp = field_datetime(Some(record), "timestamp")?;

    Some(FitTrackPoint {
        timestamp,
        latitude: field_f64(Some(record), "position_lat").map(semicircles_to_degrees),
        longitude: field_f64(Some(record), "position_long").map(semicircles_to_degrees),
        distance_meters: field_f64(Some(record), "distance"),
        elevation_meters: field_f64(Some(record), "enhanced_altitude")
            .or_else(|| field_f64(Some(record), "altitude")),
        speed_mps: field_f64(Some(record), "enhanced_speed")
            .or_else(|| field_f64(Some(record), "speed")),
        heart_rate_bpm: field_i32(Some(record), "heart_rate"),
        cadence_rpm: field_i32(Some(record), "cadence"),
        power_watts: field_i32(Some(record), "power"),
    })
}

fn parse_fit_lap(record: &FitDataRecord) -> FitLapSummary {
    FitLapSummary {
        start_time: field_datetime(Some(record), "start_time"),
        end_time: field_datetime(Some(record), "timestamp"),
        duration_seconds: field_seconds(Some(record), "total_timer_time")
            .or_else(|| field_seconds(Some(record), "total_elapsed_time")),
        distance_meters: field_f64(Some(record), "total_distance"),
        elevation_gain_meters: field_f64(Some(record), "total_ascent"),
        elevation_loss_meters: field_f64(Some(record), "total_descent"),
        average_speed_mps: field_f64(Some(record), "enhanced_avg_speed")
            .or_else(|| field_f64(Some(record), "avg_speed")),
        max_speed_mps: field_f64(Some(record), "enhanced_max_speed")
            .or_else(|| field_f64(Some(record), "max_speed")),
        average_heart_rate_bpm: field_i32(Some(record), "avg_heart_rate"),
        max_heart_rate_bpm: field_i32(Some(record), "max_heart_rate"),
        average_cadence_rpm: field_i32(Some(record), "avg_cadence"),
        max_cadence_rpm: field_i32(Some(record), "max_cadence"),
        calories: field_i32(Some(record), "total_calories"),
    }
}

fn field_value<'a>(record: Option<&'a FitDataRecord>, name: &str) -> Option<&'a Value> {
    record?
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value())
}

fn field_f64(record: Option<&FitDataRecord>, name: &str) -> Option<f64> {
    value_as_f64(field_value(record, name))
}

fn field_i32(record: Option<&FitDataRecord>, name: &str) -> Option<i32> {
    value_as_f64(field_value(record, name)).map(|value| value.round() as i32)
}

fn field_seconds(record: Option<&FitDataRecord>, name: &str) -> Option<i32> {
    value_as_f64(field_value(record, name))
        .map(|value| value.round() as i32)
        .filter(|value| *value >= 0)
}

fn field_datetime(record: Option<&FitDataRecord>, name: &str) -> Option<DateTime<Utc>> {
    match field_value(record, name) {
        Some(Value::Timestamp(value)) => Some(value.with_timezone(&Utc)),
        _ => None,
    }
}

fn field_string(record: Option<&FitDataRecord>, name: &str) -> Option<String> {
    match field_value(record, name) {
        Some(Value::String(value)) => Some(value.clone()),
        Some(value) => Some(value.to_string()),
        None => None,
    }
}

fn value_as_f64(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Byte(value) => Some(f64::from(*value)),
        Value::Enum(value) => Some(f64::from(*value)),
        Value::SInt8(value) => Some(f64::from(*value)),
        Value::UInt8(value) => Some(f64::from(*value)),
        Value::SInt16(value) => Some(f64::from(*value)),
        Value::UInt16(value) => Some(f64::from(*value)),
        Value::SInt32(value) => Some(f64::from(*value)),
        Value::UInt32(value) => Some(f64::from(*value)),
        Value::Float32(value) => Some(f64::from(*value)),
        Value::Float64(value) => Some(*value),
        Value::UInt8z(value) => Some(f64::from(*value)),
        Value::UInt16z(value) => Some(f64::from(*value)),
        Value::UInt32z(value) => Some(f64::from(*value)),
        Value::SInt64(value) => Some(*value as f64),
        Value::UInt64(value) => Some(*value as f64),
        Value::UInt64z(value) => Some(*value as f64),
        _ => None,
    }
}

fn semicircles_to_degrees(value: f64) -> f64 {
    value * SEMICIRCLES_TO_DEGREES
}

fn average_metric(values: &[i32]) -> Option<i32> {
    if values.is_empty() {
        return None;
    }

    let total = values.iter().map(|value| i64::from(*value)).sum::<i64>();
    Some((total as f64 / values.len() as f64).round() as i32)
}

fn max_metric(values: &[i32]) -> Option<i32> {
    values.iter().copied().max()
}

fn seconds_between(start: DateTime<Utc>, end: DateTime<Utc>) -> Option<i32> {
    let delta = (end - start).num_seconds();
    (delta >= 0).then_some(delta as i32)
}

fn humanize_filename(filename: &str) -> String {
    let raw = Path::new(filename)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.replace(['_', '-'], " "))
        .map(|value| value.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_else(|| "activity".to_string());

    if raw.is_empty() {
        "Activity".to_string()
    } else {
        title_case_words(&raw)
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
