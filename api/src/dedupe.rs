use crate::activity_details::{deserialize_derived_activity_data, ActivityRoutePoint};
use crate::activity_summary::ActivityDraft;
use crate::entities::{activities, segments};
use crate::segment_support::deserialize_segment_route_points;
use chrono::{DateTime, SecondsFormat, Utc};

const ACTIVITY_DISTANCE_BUCKET_METERS: f64 = 25.0;
const SEGMENT_DISTANCE_BUCKET_METERS: f64 = 5.0;
const DURATION_BUCKET_SECONDS: i32 = 5;
const COORDINATE_DECIMALS: usize = 4;
const MAX_ROUTE_SAMPLE_POINTS: usize = 16;

pub fn activity_dedupe_key(draft: &ActivityDraft, route_points: &[ActivityRoutePoint]) -> String {
    activity_dedupe_key_from_fields(
        draft.started_at,
        &draft.sport,
        draft.moving_time_seconds.or(draft.total_time_seconds),
        draft.distance_meters,
        route_points,
    )
}

pub fn activity_dedupe_key_from_model(activity: &activities::Model) -> String {
    let derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());

    activity_dedupe_key_from_fields(
        activity.started_at,
        &activity.sport,
        activity.moving_time_seconds.or(activity.total_time_seconds),
        activity.distance_meters,
        &derived.route_points,
    )
}

pub fn segment_dedupe_key(
    distance_meters: Option<f64>,
    route_points: &[ActivityRoutePoint],
) -> Option<String> {
    if route_points.len() < 2 {
        return None;
    }

    Some(format!(
        "dist:{}|sample:{}",
        bucket_distance(distance_meters, SEGMENT_DISTANCE_BUCKET_METERS),
        route_signature(route_points)
    ))
}

pub fn segment_dedupe_key_from_model(segment: &segments::Model) -> Option<String> {
    let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());
    segment_dedupe_key(segment.distance_meters, &route_points)
}

fn activity_dedupe_key_from_fields(
    started_at: DateTime<Utc>,
    sport: &str,
    duration_seconds: Option<i32>,
    distance_meters: Option<f64>,
    route_points: &[ActivityRoutePoint],
) -> String {
    format!(
        "start:{}|sport:{}|dur:{}|dist:{}|sample:{}",
        started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        normalize_sport_token(sport),
        bucket_duration(duration_seconds),
        bucket_distance(distance_meters, ACTIVITY_DISTANCE_BUCKET_METERS),
        route_signature(route_points)
    )
}

fn normalize_sport_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn bucket_duration(value: Option<i32>) -> String {
    match value.filter(|seconds| *seconds > 0) {
        Some(seconds) => {
            ((seconds / DURATION_BUCKET_SECONDS) * DURATION_BUCKET_SECONDS).to_string()
        }
        None => "na".to_string(),
    }
}

fn bucket_distance(value: Option<f64>, bucket_meters: f64) -> String {
    match value.filter(|meters| *meters > 0.0) {
        Some(meters) => ((meters / bucket_meters).floor() * bucket_meters)
            .round()
            .to_string(),
        None => "na".to_string(),
    }
}

fn route_signature(route_points: &[ActivityRoutePoint]) -> String {
    if route_points.is_empty() {
        return "na".to_string();
    }

    sample_route_points(route_points, MAX_ROUTE_SAMPLE_POINTS)
        .into_iter()
        .map(|point| {
            format!(
                "{lat:.prec$}:{lon:.prec$}",
                lat = point.latitude,
                lon = point.longitude,
                prec = COORDINATE_DECIMALS,
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn sample_route_points(
    route_points: &[ActivityRoutePoint],
    max_points: usize,
) -> Vec<&ActivityRoutePoint> {
    if route_points.len() <= max_points {
        return route_points.iter().collect();
    }

    let mut samples = Vec::with_capacity(max_points);

    for sample_index in 0..max_points {
        let index = if sample_index == max_points - 1 {
            route_points.len() - 1
        } else {
            sample_index * (route_points.len() - 1) / (max_points - 1)
        };

        if samples
            .last()
            .map(|point: &&ActivityRoutePoint| point.elapsed_seconds)
            == Some(route_points[index].elapsed_seconds)
        {
            continue;
        }

        samples.push(&route_points[index]);
    }

    samples
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_route_point(elapsed_seconds: i32, latitude: f64, longitude: f64) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds,
            latitude,
            longitude,
            distance_meters: Some(f64::from(elapsed_seconds.max(0))),
            elevation_meters: Some(100.0),
            speed_mps: Some(8.0),
            heart_rate_bpm: Some(140),
            cadence_rpm: Some(88),
        }
    }

    fn make_draft() -> ActivityDraft {
        ActivityDraft {
            title: "Lunch Ride".to_string(),
            sport: "ride".to_string(),
            started_at: DateTime::parse_from_rfc3339("2026-05-08T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            ended_at: None,
            distance_meters: Some(25234.0),
            moving_time_seconds: Some(3602),
            total_time_seconds: Some(3608),
            elevation_gain_meters: Some(450.0),
            elevation_loss_meters: Some(448.0),
            average_speed_mps: Some(7.0),
            max_speed_mps: Some(12.0),
            average_heart_rate_bpm: Some(140),
            max_heart_rate_bpm: Some(175),
            average_cadence_rpm: Some(86),
            max_cadence_rpm: Some(100),
            calories: Some(800),
        }
    }

    #[test]
    fn activity_key_buckets_small_metric_differences() {
        let draft = make_draft();
        let route = vec![
            make_route_point(0, 45.50001, -122.60001),
            make_route_point(1800, 45.60004, -122.70004),
            make_route_point(3600, 45.70001, -122.80001),
        ];

        let mut slightly_different = draft.clone();
        slightly_different.distance_meters = Some(25245.0);
        slightly_different.moving_time_seconds = Some(3604);
        let slightly_different_route = vec![
            make_route_point(0, 45.50002, -122.60002),
            make_route_point(1800, 45.60003, -122.70003),
            make_route_point(3600, 45.70002, -122.80002),
        ];

        assert_eq!(
            activity_dedupe_key(&draft, &route),
            activity_dedupe_key(&slightly_different, &slightly_different_route),
        );
    }

    #[test]
    fn segment_key_matches_same_route_geometry() {
        let route = vec![
            make_route_point(0, 45.50001, -122.60001),
            make_route_point(10, 45.51001, -122.61001),
            make_route_point(20, 45.52001, -122.62001),
        ];
        let route_with_small_jitter = vec![
            make_route_point(0, 45.50002, -122.60002),
            make_route_point(10, 45.51002, -122.61002),
            make_route_point(20, 45.52002, -122.62002),
        ];

        assert_eq!(
            segment_dedupe_key(Some(1500.0), &route),
            segment_dedupe_key(Some(1503.0), &route_with_small_jitter),
        );
    }
}
