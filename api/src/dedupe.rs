use crate::activity_details::{deserialize_derived_activity_data, ActivityRoutePoint};
use crate::activity_summary::ActivityDraft;
use crate::entities::{activities, segments};
use crate::segment_support::deserialize_segment_route_points;
use chrono::{DateTime, SecondsFormat, Utc};

const ACTIVITY_DISTANCE_BUCKET_METERS: f64 = 25.0;
const SEGMENT_DISTANCE_BUCKET_METERS: f64 = 5.0;
const DURATION_BUCKET_SECONDS: i32 = 5;
const ACTIVITY_DISTANCE_MATCH_TOLERANCE_METERS: f64 = 250.0;
const ACTIVITY_DURATION_MATCH_TOLERANCE_SECONDS: i32 = 300;
const ACTIVITY_START_LOCATION_MATCH_TOLERANCE_METERS: f64 = 2_500.0;
const COORDINATE_DECIMALS: usize = 4;
const MAX_ROUTE_SAMPLE_POINTS: usize = 16;
const ACTIVITY_ROUTE_EDGE_FRACTION: f64 = 0.20;
const ACTIVITY_ROUTE_MATCH_TOLERANCE_METERS: f64 = 1_500.0;
const ACTIVITY_ROUTE_MATCH_MIN_RATIO_NUMERATOR: usize = 3;
const ACTIVITY_ROUTE_MATCH_MIN_RATIO_DENOMINATOR: usize = 4;

pub fn activity_dedupe_key(draft: &ActivityDraft, route_points: &[ActivityRoutePoint]) -> String {
    activity_dedupe_key_from_fields(
        draft.started_at,
        &draft.sport,
        dedupe_duration_seconds(draft.total_time_seconds, draft.moving_time_seconds),
        draft.distance_meters,
        route_points,
    )
}

pub fn activity_dedupe_key_from_model(activity: &activities::Model) -> String {
    let derived = deserialize_derived_activity_data(activity.derived_data_json.as_ref());

    activity_dedupe_key_from_fields(
        activity.started_at,
        &activity.sport,
        dedupe_duration_seconds(activity.total_time_seconds, activity.moving_time_seconds),
        activity.distance_meters,
        &derived.route_points,
    )
}

pub fn activity_duplicate_candidate_key(started_at: DateTime<Utc>, sport: &str) -> String {
    format!(
        "start:{}|sport:{}",
        started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        normalize_sport_token(sport),
    )
}

pub fn activity_dedupe_matches_model(
    activity: &activities::Model,
    draft: &ActivityDraft,
    route_points: &[ActivityRoutePoint],
) -> bool {
    if !activity_started_at_and_sport_match(
        activity.started_at,
        &activity.sport,
        draft.started_at,
        &draft.sport,
    ) {
        return false;
    }

    if !activity_metrics_match(
        activity.total_time_seconds,
        activity.moving_time_seconds,
        activity.distance_meters,
        draft.total_time_seconds,
        draft.moving_time_seconds,
        draft.distance_meters,
    ) {
        return false;
    }

    let existing_route_points =
        deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
    activity_routes_match(&existing_route_points, route_points)
        || activity_location_and_distance_match(
            &existing_route_points,
            route_points,
            activity.distance_meters,
            draft.distance_meters,
        )
}

pub fn activity_models_match_for_dedupe(
    left: &activities::Model,
    right: &activities::Model,
) -> bool {
    if !activity_started_at_and_sport_match(
        left.started_at,
        &left.sport,
        right.started_at,
        &right.sport,
    ) {
        return false;
    }

    if !activity_metrics_match(
        left.total_time_seconds,
        left.moving_time_seconds,
        left.distance_meters,
        right.total_time_seconds,
        right.moving_time_seconds,
        right.distance_meters,
    ) {
        return false;
    }

    let left_route_points =
        deserialize_derived_activity_data(left.derived_data_json.as_ref()).route_points;
    let right_route_points =
        deserialize_derived_activity_data(right.derived_data_json.as_ref()).route_points;
    activity_routes_match(&left_route_points, &right_route_points)
        || activity_location_and_distance_match(
            &left_route_points,
            &right_route_points,
            left.distance_meters,
            right.distance_meters,
        )
}

fn dedupe_duration_seconds(
    total_time_seconds: Option<i32>,
    moving_time_seconds: Option<i32>,
) -> Option<i32> {
    total_time_seconds.or(moving_time_seconds)
}

fn activity_started_at_and_sport_match(
    left_started_at: DateTime<Utc>,
    left_sport: &str,
    right_started_at: DateTime<Utc>,
    right_sport: &str,
) -> bool {
    left_started_at == right_started_at
        && normalize_sport_token(left_sport) == normalize_sport_token(right_sport)
}

fn activity_metrics_match(
    left_total_time_seconds: Option<i32>,
    left_moving_time_seconds: Option<i32>,
    left_distance_meters: Option<f64>,
    right_total_time_seconds: Option<i32>,
    right_moving_time_seconds: Option<i32>,
    right_distance_meters: Option<f64>,
) -> bool {
    duration_difference_seconds(
        left_total_time_seconds,
        left_moving_time_seconds,
        right_total_time_seconds,
        right_moving_time_seconds,
    )
    .is_none_or(|difference| difference <= ACTIVITY_DURATION_MATCH_TOLERANCE_SECONDS)
        && distance_difference_meters(left_distance_meters, right_distance_meters)
            .is_none_or(|difference| difference <= ACTIVITY_DISTANCE_MATCH_TOLERANCE_METERS)
}

fn duration_difference_seconds(
    left_total_time_seconds: Option<i32>,
    left_moving_time_seconds: Option<i32>,
    right_total_time_seconds: Option<i32>,
    right_moving_time_seconds: Option<i32>,
) -> Option<i32> {
    let mut differences = [
        option_difference_i32(left_total_time_seconds, right_total_time_seconds),
        option_difference_i32(left_total_time_seconds, right_moving_time_seconds),
        option_difference_i32(left_moving_time_seconds, right_total_time_seconds),
        option_difference_i32(left_moving_time_seconds, right_moving_time_seconds),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();

    differences.sort_unstable();
    differences.into_iter().next()
}

fn distance_difference_meters(
    left_distance_meters: Option<f64>,
    right_distance_meters: Option<f64>,
) -> Option<f64> {
    match (
        left_distance_meters.filter(|value| value.is_finite()),
        right_distance_meters.filter(|value| value.is_finite()),
    ) {
        (Some(left_distance), Some(right_distance)) => Some((left_distance - right_distance).abs()),
        _ => None,
    }
}

fn option_difference_i32(left: Option<i32>, right: Option<i32>) -> Option<i32> {
    match (left, right) {
        (Some(left_value), Some(right_value)) => Some((left_value - right_value).abs()),
        _ => None,
    }
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
    _route_points: &[ActivityRoutePoint],
) -> String {
    format!(
        "start:{}|sport:{}|dur:{}|dist:{}",
        started_at.to_rfc3339_opts(SecondsFormat::Secs, true),
        normalize_sport_token(sport),
        bucket_duration(duration_seconds),
        bucket_distance(distance_meters, ACTIVITY_DISTANCE_BUCKET_METERS),
    )
}

fn activity_routes_match(left: &[ActivityRoutePoint], right: &[ActivityRoutePoint]) -> bool {
    match (left.is_empty(), right.is_empty()) {
        (true, true) => return true,
        (true, false) | (false, true) => return false,
        (false, false) => {}
    }

    let left_samples =
        interpolated_route_samples(left, MAX_ROUTE_SAMPLE_POINTS, ACTIVITY_ROUTE_EDGE_FRACTION)
            .unwrap_or_else(|| sampled_route_coordinates(left, MAX_ROUTE_SAMPLE_POINTS));
    let right_samples =
        interpolated_route_samples(right, MAX_ROUTE_SAMPLE_POINTS, ACTIVITY_ROUTE_EDGE_FRACTION)
            .unwrap_or_else(|| sampled_route_coordinates(right, MAX_ROUTE_SAMPLE_POINTS));

    let sample_count = left_samples.len().min(right_samples.len());
    if sample_count == 0 {
        return false;
    }

    let matching_samples = left_samples
        .iter()
        .zip(right_samples.iter())
        .take(sample_count)
        .filter(|((left_lat, left_lon), (right_lat, right_lon))| {
            haversine_distance_meters(*left_lat, *left_lon, *right_lat, *right_lon)
                <= ACTIVITY_ROUTE_MATCH_TOLERANCE_METERS
        })
        .count();

    matching_samples * ACTIVITY_ROUTE_MATCH_MIN_RATIO_DENOMINATOR
        >= sample_count * ACTIVITY_ROUTE_MATCH_MIN_RATIO_NUMERATOR
        || sampled_routes_overlap(&left_samples, &right_samples)
}

fn activity_location_and_distance_match(
    left_route_points: &[ActivityRoutePoint],
    right_route_points: &[ActivityRoutePoint],
    left_distance_meters: Option<f64>,
    right_distance_meters: Option<f64>,
) -> bool {
    let Some(left_start) = left_route_points.first() else {
        return false;
    };
    let Some(right_start) = right_route_points.first() else {
        return false;
    };

    haversine_distance_meters(
        left_start.latitude,
        left_start.longitude,
        right_start.latitude,
        right_start.longitude,
    ) <= ACTIVITY_START_LOCATION_MATCH_TOLERANCE_METERS
        && distance_difference_meters(left_distance_meters, right_distance_meters)
            .is_none_or(|difference| difference <= ACTIVITY_DISTANCE_MATCH_TOLERANCE_METERS)
}

fn sampled_routes_overlap(left_samples: &[(f64, f64)], right_samples: &[(f64, f64)]) -> bool {
    let (smaller_samples, larger_samples) = if left_samples.len() <= right_samples.len() {
        (left_samples, right_samples)
    } else {
        (right_samples, left_samples)
    };

    if smaller_samples.is_empty() || larger_samples.is_empty() {
        return false;
    }

    let matching_samples = smaller_samples
        .iter()
        .filter(|(sample_latitude, sample_longitude)| {
            larger_samples
                .iter()
                .any(|(candidate_latitude, candidate_longitude)| {
                    haversine_distance_meters(
                        *sample_latitude,
                        *sample_longitude,
                        *candidate_latitude,
                        *candidate_longitude,
                    ) <= ACTIVITY_ROUTE_MATCH_TOLERANCE_METERS
                })
        })
        .count();

    matching_samples * ACTIVITY_ROUTE_MATCH_MIN_RATIO_DENOMINATOR
        >= smaller_samples.len() * ACTIVITY_ROUTE_MATCH_MIN_RATIO_NUMERATOR
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

    format_route_signature(sample_route_points(route_points, MAX_ROUTE_SAMPLE_POINTS))
}

fn format_route_signature(route_points: Vec<&ActivityRoutePoint>) -> String {
    route_points
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

fn sampled_route_coordinates(
    route_points: &[ActivityRoutePoint],
    max_points: usize,
) -> Vec<(f64, f64)> {
    sample_route_points(route_points, max_points)
        .into_iter()
        .map(|point| (point.latitude, point.longitude))
        .collect()
}

fn interpolated_route_samples(
    route_points: &[ActivityRoutePoint],
    max_points: usize,
    edge_fraction: f64,
) -> Option<Vec<(f64, f64)>> {
    if route_points.is_empty() {
        return Some(Vec::new());
    }

    if route_points.len() == 1 || max_points == 0 {
        return Some(
            route_points
                .iter()
                .map(|point| (point.latitude, point.longitude))
                .collect(),
        );
    }

    let progress_values = route_progress_values(route_points)?;
    let start_progress = *progress_values.first()?;
    let end_progress = *progress_values.last()?;
    let progress_span = end_progress - start_progress;

    if progress_span <= 0.0 {
        return None;
    }

    let clamped_edge_fraction = edge_fraction.clamp(0.0, 0.49);
    let mut samples = Vec::with_capacity(max_points);
    let mut cursor = 0usize;

    for sample_index in 0..max_points {
        let sample_fraction = if max_points == 1 {
            0.5
        } else {
            clamped_edge_fraction
                + (1.0 - (clamped_edge_fraction * 2.0))
                    * (sample_index as f64 / (max_points - 1) as f64)
        };
        let target_progress = start_progress + (progress_span * sample_fraction);

        while cursor + 1 < progress_values.len() && progress_values[cursor] < target_progress {
            cursor += 1;
        }

        let (latitude, longitude) =
            interpolated_coordinate(route_points, &progress_values, cursor, target_progress);

        samples.push((latitude, longitude));
    }

    Some(samples)
}

fn interpolated_coordinate(
    route_points: &[ActivityRoutePoint],
    progress_values: &[f64],
    cursor: usize,
    target_progress: f64,
) -> (f64, f64) {
    if cursor == 0 {
        return (route_points[0].latitude, route_points[0].longitude);
    }

    let previous_index = cursor.saturating_sub(1);
    let previous_progress = progress_values[previous_index];
    let current_progress = progress_values[cursor];

    if current_progress <= previous_progress {
        return (
            route_points[cursor].latitude,
            route_points[cursor].longitude,
        );
    }

    let interpolation = ((target_progress - previous_progress)
        / (current_progress - previous_progress))
        .clamp(0.0, 1.0);
    let previous_point = &route_points[previous_index];
    let current_point = &route_points[cursor];

    (
        previous_point.latitude
            + ((current_point.latitude - previous_point.latitude) * interpolation),
        previous_point.longitude
            + ((current_point.longitude - previous_point.longitude) * interpolation),
    )
}

fn route_progress_values(route_points: &[ActivityRoutePoint]) -> Option<Vec<f64>> {
    let use_distance_progress = route_points
        .last()
        .and_then(|point| point.distance_meters)
        .filter(|distance| *distance > 0.0)
        .is_some();

    let mut progress_values = Vec::with_capacity(route_points.len());
    let mut last_progress = 0.0;

    for point in route_points {
        let raw_progress = if use_distance_progress {
            point.distance_meters?
        } else {
            f64::from(point.elapsed_seconds.max(0))
        };

        if !raw_progress.is_finite() {
            return None;
        }

        last_progress = raw_progress.max(last_progress);
        progress_values.push(last_progress);
    }

    Some(progress_values)
}

fn haversine_distance_meters(
    latitude_a: f64,
    longitude_a: f64,
    latitude_b: f64,
    longitude_b: f64,
) -> f64 {
    let latitude_a = latitude_a.to_radians();
    let latitude_b = latitude_b.to_radians();
    let delta_latitude = (latitude_b - latitude_a) / 2.0;
    let delta_longitude = (longitude_b.to_radians() - longitude_a.to_radians()) / 2.0;

    let haversine = delta_latitude.sin().powi(2)
        + latitude_a.cos() * latitude_b.cos() * delta_longitude.sin().powi(2);
    let angular_distance = 2.0 * haversine.sqrt().asin();

    6_371_000.0 * angular_distance
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
            power_watts: None,
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
    fn activity_key_prefers_total_duration_over_moving_duration() {
        let draft = make_draft();
        let route = vec![
            make_route_point(0, 45.50001, -122.60001),
            make_route_point(1800, 45.60004, -122.70004),
            make_route_point(3600, 45.70001, -122.80001),
        ];

        let mut source_variant = draft.clone();
        source_variant.moving_time_seconds = Some(3611);
        source_variant.total_time_seconds = Some(3608);
        source_variant.distance_meters = Some(25240.0);

        assert_eq!(
            activity_dedupe_key(&draft, &route),
            activity_dedupe_key(&source_variant, &route),
        );
    }

    #[test]
    fn activity_key_matches_same_route_with_different_sampling_density() {
        let anchors = [
            (45.5000, -122.6000),
            (45.6200, -122.7200),
            (45.7100, -122.8100),
            (45.7600, -122.7300),
            (45.6900, -122.6200),
        ];

        let dense_route = build_route(&anchors, 120, None, None);
        let sparse_route = build_route(
            &anchors,
            90,
            Some((45.4920, -122.5880)),
            Some((45.6970, -122.6110)),
        );

        assert!(activity_routes_match(&dense_route, &sparse_route));
    }

    #[test]
    fn activity_routes_match_when_one_source_trims_start_and_end() {
        let anchors = [
            (44.7539, -85.6290),
            (44.7600, -85.6000),
            (44.7420, -85.5109),
        ];

        let full_route = build_route(&anchors, 240, None, None);
        let trimmed_route = build_route(
            &anchors,
            180,
            Some((44.7552, -85.6176)),
            Some((44.7414, -85.5091)),
        );

        assert!(activity_routes_match(&full_route, &trimmed_route));
    }

    #[test]
    fn activity_matches_same_start_location_and_near_distance() {
        let left_route = vec![
            make_route_point(0, 44.7552, -85.6176),
            make_route_point(1800, 44.7600, -85.6000),
            make_route_point(3300, 44.7414, -85.5091),
        ];
        let right_route = vec![
            make_route_point(295, 44.7553, -85.6177),
            make_route_point(1800, 44.7598, -85.5999),
            make_route_point(3600, 44.7421, -85.5108),
        ];

        assert!(activity_location_and_distance_match(
            &left_route,
            &right_route,
            Some(28_396.9),
            Some(28_396.91),
        ));
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

    fn build_route(
        anchors: &[(f64, f64)],
        points_per_leg: usize,
        start_override: Option<(f64, f64)>,
        end_override: Option<(f64, f64)>,
    ) -> Vec<ActivityRoutePoint> {
        let mut coordinates = Vec::new();

        if let Some((latitude, longitude)) = start_override {
            coordinates.push((latitude, longitude));
        }

        for (leg_index, window) in anchors.windows(2).enumerate() {
            let (start_lat, start_lon) = window[0];
            let (end_lat, end_lon) = window[1];
            let start_step = usize::from(leg_index > 0);

            for step in start_step..=points_per_leg {
                let fraction = step as f64 / points_per_leg as f64;
                let latitude = start_lat + ((end_lat - start_lat) * fraction);
                let longitude = start_lon + ((end_lon - start_lon) * fraction);

                coordinates.push((latitude, longitude));
            }
        }

        if let Some((latitude, longitude)) = end_override {
            coordinates.push((latitude, longitude));
        }

        let total_steps = (coordinates.len().saturating_sub(1)).max(1) as f64;

        coordinates
            .into_iter()
            .enumerate()
            .map(|(index, (latitude, longitude))| {
                let fraction = index as f64 / total_steps;

                ActivityRoutePoint {
                    elapsed_seconds: (fraction * 3608.0).round() as i32,
                    latitude,
                    longitude,
                    distance_meters: Some(fraction * 25234.0),
                    elevation_meters: Some(100.0),
                    speed_mps: Some(8.0),
                    heart_rate_bpm: Some(140),
                    cadence_rpm: Some(88),
                    power_watts: None,
                }
            })
            .collect()
    }
}
