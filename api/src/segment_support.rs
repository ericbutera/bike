use crate::activity_details::{deserialize_derived_activity_data, ActivityRoutePoint};
use crate::app_error::AppError;
use crate::entities::{activities, segment_efforts, segments};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};

const MIN_ENDPOINT_THRESHOLD_METERS: f64 = 50.0;
const MAX_ENDPOINT_THRESHOLD_METERS: f64 = 140.0;
const ENDPOINT_THRESHOLD_SPACING_MULTIPLIER: f64 = 0.7;
const SHAPE_AVERAGE_THRESHOLD_METERS: f64 = 55.0;
const SHAPE_MAX_THRESHOLD_METERS: f64 = 110.0;
const DISTANCE_RATIO_MIN: f64 = 0.65;
const DISTANCE_RATIO_MAX: f64 = 1.35;
const SHAPE_SAMPLE_POINTS: usize = 12;

#[derive(Debug, Clone, PartialEq)]
struct MatchedSegmentEffort {
    start_route_point_index: i32,
    end_route_point_index: i32,
    start_elapsed_seconds: i32,
    end_elapsed_seconds: i32,
    duration_seconds: i32,
    distance_meters: Option<f64>,
}

pub fn serialize_segment_route_points(
    route_points: &[ActivityRoutePoint],
) -> Result<String, AppError> {
    serde_json::to_string(route_points).map_err(|error| {
        tracing::error!(error = ?error, "failed to serialize segment route points");
        AppError::internal("Failed to serialize segment route data")
    })
}

pub fn deserialize_segment_route_points(raw: Option<&str>) -> Vec<ActivityRoutePoint> {
    match raw {
        Some(value) if !value.trim().is_empty() => {
            serde_json::from_str(value).unwrap_or_else(|error| {
                tracing::warn!(error = ?error, "failed to deserialize segment route points");
                Vec::new()
            })
        }
        _ => Vec::new(),
    }
}

pub fn slice_effort_route_points(
    route_points: &[ActivityRoutePoint],
    start_route_point_index: i32,
    end_route_point_index: i32,
) -> Vec<ActivityRoutePoint> {
    let start_index = usize::try_from(start_route_point_index).ok();
    let end_index = usize::try_from(end_route_point_index).ok();
    let (Some(start_index), Some(end_index)) = (start_index, end_index) else {
        return Vec::new();
    };

    if start_index >= route_points.len()
        || end_index >= route_points.len()
        || start_index > end_index
    {
        return Vec::new();
    }

    let start_elapsed_seconds = route_points[start_index].elapsed_seconds;
    let start_distance_meters = route_points[start_index].distance_meters;

    route_points[start_index..=end_index]
        .iter()
        .map(|point| ActivityRoutePoint {
            elapsed_seconds: point.elapsed_seconds.saturating_sub(start_elapsed_seconds),
            latitude: point.latitude,
            longitude: point.longitude,
            distance_meters: normalize_distance(point.distance_meters, start_distance_meters),
            elevation_meters: point.elevation_meters,
            speed_mps: point.speed_mps,
            heart_rate_bpm: point.heart_rate_bpm,
            cadence_rpm: point.cadence_rpm,
        })
        .collect()
}

pub async fn clear_segment_efforts_for_activity<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    segment_efforts::Entity::delete_many()
        .filter(segment_efforts::Column::UserId.eq(user_id))
        .filter(segment_efforts::Column::ActivityId.eq(activity_id))
        .exec(db)
        .await?;

    Ok(())
}

pub async fn replace_segment_efforts_for_activity<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    activity_route_points: &[ActivityRoutePoint],
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    clear_segment_efforts_for_activity(db, user_id, activity_id).await?;

    if activity_route_points.len() < 2 {
        return Ok(());
    }

    let segments = segments::Entity::find().all(db).await?;

    for segment in segments {
        let segment_route_points =
            deserialize_segment_route_points(segment.route_data_json.as_deref());
        if segment_route_points.len() < 2 {
            continue;
        }

        let matches = match_segment_efforts(&segment_route_points, activity_route_points);
        insert_matches(db, user_id, segment.id, activity_id, &matches).await?;
    }

    Ok(())
}

pub async fn replace_segment_efforts_for_segment<C>(
    db: &C,
    segment_id: i32,
    segment_route_points: &[ActivityRoutePoint],
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    segment_efforts::Entity::delete_many()
        .filter(segment_efforts::Column::SegmentId.eq(segment_id))
        .exec(db)
        .await?;

    if segment_route_points.len() < 2 {
        return Ok(());
    }

    let activities = activities::Entity::find().all(db).await?;

    for activity in activities {
        let route_points =
            deserialize_derived_activity_data(activity.derived_data_json.as_deref()).route_points;
        if route_points.len() < 2 {
            continue;
        }

        let matches = match_segment_efforts(segment_route_points, &route_points);
        insert_matches(db, activity.user_id, segment_id, activity.id, &matches).await?;
    }

    Ok(())
}

async fn insert_matches<C>(
    db: &C,
    user_id: i32,
    segment_id: i32,
    activity_id: i32,
    matches: &[MatchedSegmentEffort],
) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    for (index, matched_effort) in matches.iter().enumerate() {
        segment_efforts::ActiveModel {
            user_id: Set(user_id),
            segment_id: Set(segment_id),
            activity_id: Set(activity_id),
            effort_index: Set((index + 1) as i32),
            start_route_point_index: Set(matched_effort.start_route_point_index),
            end_route_point_index: Set(matched_effort.end_route_point_index),
            start_elapsed_seconds: Set(matched_effort.start_elapsed_seconds),
            end_elapsed_seconds: Set(matched_effort.end_elapsed_seconds),
            duration_seconds: Set(matched_effort.duration_seconds),
            distance_meters: Set(matched_effort.distance_meters),
            ..Default::default()
        }
        .insert(db)
        .await?;
    }

    Ok(())
}

fn match_segment_efforts(
    segment_route_points: &[ActivityRoutePoint],
    activity_route_points: &[ActivityRoutePoint],
) -> Vec<MatchedSegmentEffort> {
    if segment_route_points.len() < 2 || activity_route_points.len() < 2 {
        return Vec::new();
    }

    let segment_distance_meters = route_distance_meters(segment_route_points);
    let segment_start = &segment_route_points[0];
    let segment_end = &segment_route_points[segment_route_points.len() - 1];
    let endpoint_threshold_meters =
        derive_endpoint_threshold_meters(segment_route_points, activity_route_points);
    let mut matches = Vec::new();
    let mut start_index = 0usize;

    while start_index + 1 < activity_route_points.len() {
        let candidate_start = &activity_route_points[start_index];
        let start_error_meters = haversine_distance_meters(
            candidate_start.latitude,
            candidate_start.longitude,
            segment_start.latitude,
            segment_start.longitude,
        );

        if start_error_meters > endpoint_threshold_meters {
            start_index += 1;
            continue;
        }

        let mut best_match: Option<(f64, usize, Option<f64>)> = None;

        for end_index in (start_index + 1)..activity_route_points.len() {
            let candidate_end = &activity_route_points[end_index];
            let end_error_meters = haversine_distance_meters(
                candidate_end.latitude,
                candidate_end.longitude,
                segment_end.latitude,
                segment_end.longitude,
            );

            if end_error_meters > endpoint_threshold_meters {
                continue;
            }

            let duration_seconds = candidate_end.elapsed_seconds - candidate_start.elapsed_seconds;
            if duration_seconds <= 0 {
                continue;
            }

            let candidate_distance_meters =
                route_distance_between(activity_route_points, start_index, end_index);
            if !distance_ratio_within_bounds(segment_distance_meters, candidate_distance_meters) {
                continue;
            }

            let activity_slice = &activity_route_points[start_index..=end_index];
            let (average_shape_error_meters, max_shape_error_meters) =
                shape_error_meters(segment_route_points, activity_slice);
            if average_shape_error_meters > SHAPE_AVERAGE_THRESHOLD_METERS
                || max_shape_error_meters > SHAPE_MAX_THRESHOLD_METERS
            {
                continue;
            }

            let distance_penalty =
                distance_ratio_penalty(segment_distance_meters, candidate_distance_meters);
            let score = average_shape_error_meters
                + max_shape_error_meters * 0.15
                + start_error_meters * 0.35
                + end_error_meters * 0.35
                + distance_penalty;

            match best_match {
                Some((best_score, _, _)) if best_score <= score => {}
                _ => best_match = Some((score, end_index, candidate_distance_meters)),
            }
        }

        if let Some((_, end_index, distance_meters)) = best_match {
            let candidate_end = &activity_route_points[end_index];
            matches.push(MatchedSegmentEffort {
                start_route_point_index: start_index as i32,
                end_route_point_index: end_index as i32,
                start_elapsed_seconds: candidate_start.elapsed_seconds,
                end_elapsed_seconds: candidate_end.elapsed_seconds,
                duration_seconds: candidate_end.elapsed_seconds - candidate_start.elapsed_seconds,
                distance_meters,
            });
            start_index = end_index.saturating_add(1);
        } else {
            start_index += 1;
        }
    }

    matches
}

fn derive_endpoint_threshold_meters(
    segment_route_points: &[ActivityRoutePoint],
    activity_route_points: &[ActivityRoutePoint],
) -> f64 {
    let segment_spacing_meters = average_point_spacing_meters(segment_route_points);
    let activity_spacing_meters = average_point_spacing_meters(activity_route_points);
    let scaled_threshold_meters =
        segment_spacing_meters.max(activity_spacing_meters) * ENDPOINT_THRESHOLD_SPACING_MULTIPLIER;

    scaled_threshold_meters.clamp(MIN_ENDPOINT_THRESHOLD_METERS, MAX_ENDPOINT_THRESHOLD_METERS)
}

fn average_point_spacing_meters(route_points: &[ActivityRoutePoint]) -> f64 {
    if route_points.len() < 2 {
        return 0.0;
    }

    route_distance_meters(route_points)
        .map(|distance_meters| distance_meters / (route_points.len() - 1) as f64)
        .unwrap_or(0.0)
}

fn shape_error_meters(
    segment_route_points: &[ActivityRoutePoint],
    activity_slice: &[ActivityRoutePoint],
) -> (f64, f64) {
    let sample_count = SHAPE_SAMPLE_POINTS
        .min(segment_route_points.len())
        .min(activity_slice.len());

    if sample_count < 2 {
        return (f64::INFINITY, f64::INFINITY);
    }

    let mut total_error_meters = 0.0_f64;
    let mut max_error_meters = 0.0_f64;

    for sample_index in 0..sample_count {
        let segment_index =
            distribute_index(sample_index, sample_count, segment_route_points.len());
        let activity_index = distribute_index(sample_index, sample_count, activity_slice.len());
        let segment_point = &segment_route_points[segment_index];
        let activity_point = &activity_slice[activity_index];
        let error_meters = haversine_distance_meters(
            segment_point.latitude,
            segment_point.longitude,
            activity_point.latitude,
            activity_point.longitude,
        );

        total_error_meters += error_meters;
        max_error_meters = max_error_meters.max(error_meters);
    }

    (total_error_meters / sample_count as f64, max_error_meters)
}

fn distribute_index(sample_index: usize, sample_count: usize, point_count: usize) -> usize {
    if sample_count <= 1 || point_count <= 1 {
        return 0;
    }

    sample_index * (point_count - 1) / (sample_count - 1)
}

fn distance_ratio_within_bounds(
    segment_distance_meters: Option<f64>,
    candidate_distance_meters: Option<f64>,
) -> bool {
    let (Some(segment_distance_meters), Some(candidate_distance_meters)) =
        (segment_distance_meters, candidate_distance_meters)
    else {
        return true;
    };

    if segment_distance_meters <= 0.0 {
        return true;
    }

    let ratio = candidate_distance_meters / segment_distance_meters;
    (DISTANCE_RATIO_MIN..=DISTANCE_RATIO_MAX).contains(&ratio)
}

fn distance_ratio_penalty(
    segment_distance_meters: Option<f64>,
    candidate_distance_meters: Option<f64>,
) -> f64 {
    let (Some(segment_distance_meters), Some(candidate_distance_meters)) =
        (segment_distance_meters, candidate_distance_meters)
    else {
        return 0.0;
    };

    if segment_distance_meters <= 0.0 {
        return 0.0;
    }

    ((candidate_distance_meters / segment_distance_meters) - 1.0).abs() * 40.0
}

fn route_distance_meters(route_points: &[ActivityRoutePoint]) -> Option<f64> {
    route_distance_between(route_points, 0, route_points.len().saturating_sub(1))
}

fn route_distance_between(
    route_points: &[ActivityRoutePoint],
    start_index: usize,
    end_index: usize,
) -> Option<f64> {
    if route_points.is_empty()
        || start_index >= route_points.len()
        || end_index >= route_points.len()
        || start_index >= end_index
    {
        return None;
    }

    let start_distance_meters = route_points[start_index].distance_meters;
    let end_distance_meters = route_points[end_index].distance_meters;
    if let (Some(start_distance_meters), Some(end_distance_meters)) =
        (start_distance_meters, end_distance_meters)
    {
        if end_distance_meters >= start_distance_meters {
            return Some(end_distance_meters - start_distance_meters);
        }
    }

    let mut total_distance_meters = 0.0;
    for window in route_points[start_index..=end_index].windows(2) {
        total_distance_meters += haversine_distance_meters(
            window[0].latitude,
            window[0].longitude,
            window[1].latitude,
            window[1].longitude,
        );
    }

    Some(total_distance_meters)
}

fn normalize_distance(
    distance_meters: Option<f64>,
    start_distance_meters: Option<f64>,
) -> Option<f64> {
    match (distance_meters, start_distance_meters) {
        (Some(distance_meters), Some(start_distance_meters))
            if distance_meters >= start_distance_meters =>
        {
            Some(distance_meters - start_distance_meters)
        }
        (Some(distance_meters), _) => Some(distance_meters),
        _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn route_point(
        elapsed_seconds: i32,
        latitude: f64,
        longitude: f64,
        distance_meters: f64,
    ) -> ActivityRoutePoint {
        ActivityRoutePoint {
            elapsed_seconds,
            latitude,
            longitude,
            distance_meters: Some(distance_meters),
            elevation_meters: Some(100.0),
            speed_mps: Some(5.0),
            heart_rate_bpm: Some(140),
            cadence_rpm: Some(88),
        }
    }

    #[test]
    fn matches_repeated_segment_efforts_within_one_activity() {
        let segment_route_points = vec![
            route_point(0, 35.0000, -120.0000, 0.0),
            route_point(30, 35.0004, -120.0004, 60.0),
            route_point(60, 35.0008, -120.0008, 120.0),
        ];
        let activity_route_points = vec![
            route_point(0, 34.9995, -119.9995, 0.0),
            route_point(30, 35.0000, -120.0000, 40.0),
            route_point(60, 35.0004, -120.0004, 100.0),
            route_point(90, 35.0008, -120.0008, 160.0),
            route_point(120, 35.0012, -120.0012, 220.0),
            route_point(150, 35.0000, -120.0000, 260.0),
            route_point(180, 35.0004, -120.0004, 320.0),
            route_point(210, 35.0008, -120.0008, 380.0),
        ];

        let matches = match_segment_efforts(&segment_route_points, &activity_route_points);

        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].duration_seconds, 60);
        assert_eq!(matches[1].start_route_point_index, 5);
    }

    #[test]
    fn matches_sparse_activity_samples_near_segment_endpoints() {
        let segment_route_points = vec![
            route_point(0, 35.00048, -120.0000, 0.0),
            route_point(30, 35.00100, -120.0000, 58.0),
            route_point(60, 35.00200, -120.0000, 169.0),
            route_point(90, 35.00300, -120.0000, 280.0),
            route_point(120, 35.00352, -120.0000, 338.0),
        ];
        let activity_route_points = vec![
            route_point(0, 35.00000, -120.0000, 0.0),
            route_point(30, 35.00100, -120.0000, 111.0),
            route_point(60, 35.00200, -120.0000, 222.0),
            route_point(90, 35.00300, -120.0000, 333.0),
            route_point(120, 35.00400, -120.0000, 444.0),
        ];

        let matches = match_segment_efforts(&segment_route_points, &activity_route_points);

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_route_point_index, 0);
        assert_eq!(matches[0].end_route_point_index, 3);
        assert_eq!(matches[0].duration_seconds, 90);
    }

    #[test]
    fn slices_effort_route_points_into_local_time_and_distance() {
        let route_points = vec![
            route_point(60, 35.0000, -120.0000, 200.0),
            route_point(90, 35.0004, -120.0004, 260.0),
            route_point(120, 35.0008, -120.0008, 320.0),
        ];

        let sliced = slice_effort_route_points(&route_points, 0, 2);

        assert_eq!(sliced.len(), 3);
        assert_eq!(sliced[0].elapsed_seconds, 0);
        assert_eq!(sliced[2].elapsed_seconds, 60);
        assert_eq!(sliced[0].distance_meters, Some(0.0));
        assert_eq!(sliced[2].distance_meters, Some(120.0));
    }
}
