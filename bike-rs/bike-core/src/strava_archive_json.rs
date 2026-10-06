//! Parse authentic Strava phone recordings without generating an intermediate file.
use crate::activity_parser::ParsedActivityData;
use crate::strava_provider_payload::{parse_strava_provider_payload, StoredStravaProviderPayload};
use crate::workflow_error::WorkflowError as AppError;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
struct PhoneRecording {
    metadata: serde_json::Map<String, Value>,
    data: Vec<Series>,
}

#[derive(Deserialize)]
struct Series {
    fields: Vec<String>,
    values: Vec<Vec<Value>>,
}

pub fn parse(bytes: &[u8]) -> Result<ParsedActivityData, AppError> {
    let recording: PhoneRecording = serde_json::from_slice(bytes).map_err(|error| {
        AppError::bad_request(format!("Invalid Strava archive recording: {error}"))
    })?;
    let mut summary = recording.metadata;
    summary.insert(
        "id".into(),
        summary.get("live_activity_id").cloned().unwrap_or(json!(0)),
    );
    for (target, source) in [
        ("name", "activity_name"),
        ("sport_type", "activity_type"),
        ("moving_time", "timer_time"),
    ] {
        if let Some(value) = summary.get(source).cloned() {
            summary.insert(target.into(), value);
        }
    }
    let track = recording
        .data
        .iter()
        .find(|series| series.fields.iter().any(|field| field == "latlng"))
        .ok_or_else(|| AppError::bad_request("Strava archive recording has no GPS series"))?;
    let started_at = summary
        .get("start_date")
        .and_then(Value::as_str)
        .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        .ok_or_else(|| AppError::bad_request("Strava archive recording has no valid start date"))?;
    let streams = phone_streams(track, started_at.timestamp() as f64)?;
    let payload: StoredStravaProviderPayload =
        serde_json::from_value(json!({"provider":"strava", "activity":summary, "streams":streams}))
            .map_err(|error| {
                AppError::bad_request(format!("Invalid Strava archive metadata: {error}"))
            })?;
    // Reuse the native summary/stream normalizer. Only the authentic input bytes
    // are retained; this adapter creates no export or replacement source file.
    parse_strava_provider_payload(
        &serde_json::to_vec(&payload).map_err(|error| AppError::internal(error.to_string()))?,
    )
}

fn phone_streams(series: &Series, start_epoch: f64) -> Result<Value, AppError> {
    let mut streams = serde_json::Map::new();
    for (field, target) in [
        ("time", "time"),
        ("latlng", "latlng"),
        ("elevation", "altitude"),
        ("speed", "velocity_smooth"),
        ("distance", "distance"),
        ("heartrate", "heartrate"),
        ("cadence", "cadence"),
        ("watts", "watts"),
    ] {
        let Some(index) = series.fields.iter().position(|name| name == field) else {
            continue;
        };
        let values = series
            .values
            .iter()
            .map(|row| {
                let value = row
                    .get(index)
                    .ok_or_else(|| AppError::bad_request("Incomplete Strava archive sample"))?;
                if field != "time" {
                    return Ok(value.clone());
                }
                let time = value
                    .as_f64()
                    .ok_or_else(|| AppError::bad_request("Invalid Strava archive elapsed time"))?;
                let seconds = if time > 1_000_000_000.0 {
                    time - start_epoch
                } else {
                    time
                };
                let seconds = Some(seconds)
                    .filter(|v| v.is_finite() && *v >= 0.0 && *v <= f64::from(i32::MAX))
                    .ok_or_else(|| AppError::bad_request("Invalid Strava archive elapsed time"))?;
                Ok(json!(seconds.floor() as i32))
            })
            .collect::<Result<Vec<_>, AppError>>()?;
        streams.insert(target.into(), json!({"data":values}));
    }
    if !streams.contains_key("time") {
        return Err(AppError::bad_request(
            "Strava archive recording has no elapsed times",
        ));
    }
    Ok(Value::Object(streams))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_phone_json_preserves_real_gps_and_excludes_virtual_type() {
        for virtual_ride in [false, true] {
            let bytes = serde_json::to_vec(&json!({
                "metadata":{"activity_name":"Synthetic ride", "activity_type":if virtual_ride {"VirtualRide"} else {"Ride"}, "start_date":"2020-01-01T00:00:00Z", "timer_time":7, "elapsed_time":7},
                "data":[{"fields":["time","latlng","elevation","speed","distance"], "values":(0..8).map(|i| json!([1_577_836_800.0+f64::from(i)+0.25,[45.0,-120.0+f64::from(i)*0.0002],100,8,i*8])).collect::<Vec<_>>()}]
            })).unwrap();
            let parsed = parse(&bytes).unwrap();
            assert_eq!(parsed.derived_data.route_points.len(), 8);
            assert_eq!(parsed.derived_data.route_points[7].elapsed_seconds, 7);
            assert_eq!(
                crate::heatmaps::preparation::prepare_route(&parsed.derived_data).is_empty(),
                virtual_ride
            );
            assert_eq!(parsed.derived_data.recording.excluded(), virtual_ride);
        }
    }

    #[test]
    fn malformed_phone_tracks_fail_without_inventing_coordinates() {
        assert!(parse(br#"{"metadata":{},"data":[]}"#).is_err());
        assert!(phone_streams(
            &Series {
                fields: vec!["time".into()],
                values: vec![vec![json!(-1)]]
            },
            0.0
        )
        .is_err());
        assert!(phone_streams(
            &Series {
                fields: vec!["time".into()],
                values: vec![vec![]]
            },
            0.0
        )
        .is_err());
    }
}
