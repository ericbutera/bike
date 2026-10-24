//! Saved climb output shared by imports, reprocessing and the one-off backfill.
use crate::{
    activity_data::{deserialize_derived_activity_data, ActivityDerivedData, ActivityRoutePoint},
    bike_tasks::ActivityTask,
    entities::{activities, activity_climbs},
    object_store::{checksum, ObjectStore},
    services::reports,
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const DETECTOR_VERSION: i32 = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct Climb {
    pub climb_number: i32,
    pub start_route_point_index: Option<i32>,
    pub end_route_point_index: Option<i32>,
    pub start_elapsed_seconds: i32,
    pub end_elapsed_seconds: i32,
    pub duration_seconds: i32,
    pub distance_meters: f64,
    pub elevation_gain_meters: f64,
    pub average_grade_percent: Option<f64>,
    #[serde(default)]
    pub elevation_loss_meters: Option<f64>,
    #[serde(default)]
    pub max_grade_percent: Option<f64>,
    #[serde(default)]
    pub category: Option<String>,
    pub route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ClimbOutput {
    pub detector_version: i32,
    pub recording_sha256: String,
    pub climbs: Vec<Climb>,
}

/// Return saved climb rows; recording and processing metadata do not gate reads.
pub async fn load(db: &DatabaseConnection, activity: &activities::Model) -> Result<Option<ClimbOutput>, WorkflowError> {
    let output = activity_climbs::output::Model::for_activity(db, activity.id).await?;
    let rows = activity_climbs::Model::for_activity(db, activity.id).await?;
    Ok(Some(ClimbOutput {
        detector_version: output.as_ref().map_or(0, |output| output.detector_version),
        recording_sha256: output.map_or_else(String::new, |output| output.recording_sha256),
        climbs: rows.into_iter().map(activity_climbs::Model::into_climb).collect(),
    }))
}

fn recording_checksum(recording: &ActivityDerivedData) -> Result<String, WorkflowError> {
    serde_json::to_vec(recording)
        .map(|bytes| checksum(&bytes))
        .map_err(|error| WorkflowError::internal(error.to_string()))
}

fn climb_from_detection(climb: &reports::ClimbResponse, route: &[ActivityRoutePoint]) -> Climb {
    let start = route
        .iter()
        .position(|point| point.elapsed_seconds >= climb.start_seconds);
    let end = route
        .iter()
        .rposition(|point| point.elapsed_seconds <= climb.summit_seconds);
    let bounds = start.zip(end).filter(|(start, end)| start <= end);
    let route_points = bounds.map_or_else(Vec::new, |(start, end)| route[start..=end].to_vec());
    Climb {
        elevation_loss_meters: climb_elevation_loss(&route_points),
        max_grade_percent: climb_max_grade(&route_points),
        category: climb_category(climb.distance_meters, climb.average_grade_percent),
        climb_number: climb.climb_number,
        start_route_point_index: bounds.map(|(start, _)| start as i32),
        end_route_point_index: bounds.map(|(_, end)| end as i32),
        start_elapsed_seconds: climb.start_seconds,
        end_elapsed_seconds: climb.summit_seconds,
        duration_seconds: climb.duration_seconds,
        distance_meters: climb.distance_meters,
        elevation_gain_meters: climb.gain_meters,
        average_grade_percent: climb.average_grade_percent,
        route_points,
    }
}

fn climb_category(distance: f64, average_grade: Option<f64>) -> Option<String> {
    let grade = average_grade?;
    if distance < 250.0 || grade < 3.0 {
        return None;
    }
    let score = distance * grade;
    [
        (64_000.0, "HC"),
        (48_000.0, "1"),
        (32_000.0, "2"),
        (16_000.0, "3"),
        (8_000.0, "4"),
    ]
    .into_iter()
    .find(|(minimum, _)| score >= *minimum)
    .map(|(_, category)| category.into())
}

fn climb_elevation_loss(points: &[ActivityRoutePoint]) -> Option<f64> {
    (points.len() >= 2).then(|| {
        points
            .windows(2)
            .filter_map(|pair| {
                let previous = pair[0].elevation_meters?;
                let current = pair[1].elevation_meters?;
                (previous.is_finite() && current.is_finite()).then_some((previous - current).max(0.0))
            })
            .sum()
    })
}

fn distance_and_elevation(point: &ActivityRoutePoint) -> Option<(f64, f64)> {
    let distance = point.distance_meters?;
    let elevation = point.elevation_meters?;
    (distance.is_finite() && elevation.is_finite()).then_some((distance, elevation))
}

fn window_grade(points: &[ActivityRoutePoint], start_index: usize) -> Option<f64> {
    let (start_distance, start_elevation) = distance_and_elevation(&points[start_index])?;
    let (distance, elevation) = points[start_index + 1..]
        .iter()
        .filter_map(distance_and_elevation)
        .find(|(distance, _)| distance - start_distance >= 60.0)?;
    let grade = (elevation - start_elevation) / (distance - start_distance) * 100.0;
    grade.is_finite().then_some(grade)
}

fn interval_grade(pair: &[ActivityRoutePoint]) -> Option<f64> {
    let (start_distance, start_elevation) = distance_and_elevation(&pair[0])?;
    let (end_distance, end_elevation) = distance_and_elevation(&pair[1])?;
    let distance = end_distance - start_distance;
    if distance <= 0.0 || pair[1].elapsed_seconds <= pair[0].elapsed_seconds {
        return None;
    }
    Some((end_elevation - start_elevation) / distance * 100.0)
}

fn climb_max_grade(points: &[ActivityRoutePoint]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let window_max = (0..points.len() - 1)
        .filter_map(|index| window_grade(points, index))
        .reduce(f64::max);
    let max = window_max.or_else(|| points.windows(2).filter_map(interval_grade).reduce(f64::max));
    Some(max.unwrap_or(0.0).max(0.0))
}

pub async fn rebuild(db: &DatabaseConnection, objects: &ObjectStore, task: ActivityTask) -> Result<(), WorkflowError> {
    let Some(activity) = activities::Model::find_owned(db, task.activity_id, task.user_id).await? else {
        return Ok(());
    };
    let recording = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let recording_sha256 = recording_checksum(&recording)?;
    if activity_climbs::output::Model::for_activity(db, activity.id)
        .await?
        .is_some_and(|output| output.matches(&recording_sha256))
    {
        return Ok(());
    }
    let output = PreparedOutput {
        detector_version: DETECTOR_VERSION,
        recording_sha256,
        climbs: reports::activity_climbs(&activity)
            .into_iter()
            .map(|climb| PreparedClimb::from_detection(climb, &recording.route_points))
            .collect(),
    };
    let reference = objects
        .transform(
            &format!("activity-imports/derived/{}/climbs/v{DETECTOR_VERSION}", activity.id),
            &output,
        )
        .await?;
    activity_climbs::Model::replace_output(db, &activity, &output, &reference).await
}

mod prepared;
pub(crate) use prepared::{PreparedClimb, PreparedOutput};

#[cfg(test)]
mod tests;
