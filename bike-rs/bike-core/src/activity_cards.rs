//! Prepared activity-card location and bounded route previews.
use crate::{
    activity_data::{deserialize_derived_activity_data, ActivityRoutePoint},
    bike_tasks::ActivityTask,
    entities::{activities, activity_cards},
    workflow_error::WorkflowError,
};
use once_cell::sync::Lazy;
use reverse_geocoder::{Record, ReverseGeocoder};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use utoipa::ToSchema;

pub const MAX_PREVIEW_POINTS: usize = 24;
static REVERSE_GEOCODER: Lazy<ReverseGeocoder> = Lazy::new(ReverseGeocoder::new);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityCardPreviewPoint {
    pub elapsed_seconds: i32,
    pub latitude: f64,
    pub longitude: f64,
}

impl From<&ActivityRoutePoint> for ActivityCardPreviewPoint {
    fn from(point: &ActivityRoutePoint) -> Self {
        Self {
            elapsed_seconds: point.elapsed_seconds,
            latitude: point.latitude,
            longitude: point.longitude,
        }
    }
}

pub async fn rebuild(db: &DatabaseConnection, task: ActivityTask) -> Result<(), WorkflowError> {
    let Some(activity) = activities::Model::find_owned(db, task.activity_id, task.user_id).await? else {
        return Ok(());
    };
    let recording = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    let route_location = location_from_route_points(&recording.route_points);
    let location = activity
        .derived_data_json
        .as_ref()
        .and_then(|data| data.source_metadata())
        .and_then(|metadata| metadata.location.clone())
        .or_else(|| route_location.clone());
    let points = preview_route_points(&recording.route_points)
        .iter()
        .map(ActivityCardPreviewPoint::from)
        .collect::<Vec<_>>();
    activity_cards::Model::replace_output(db, &activity, route_location, location, &points).await
}

pub fn location_from_route_points(route_points: &[ActivityRoutePoint]) -> Option<String> {
    let point = route_points.first()?;
    let result = REVERSE_GEOCODER.search((point.latitude, point.longitude));

    format_record(result.record)
}

fn format_record(record: &Record) -> Option<String> {
    let name = non_empty(&record.name)?;
    let admin1 = non_empty(&record.admin1);
    let cc = non_empty(&record.cc);

    if let Some(admin1) = admin1.filter(|admin1| !admin1.eq_ignore_ascii_case(name)) {
        return Some(format!("{name}, {admin1}"));
    }

    if let Some(cc) = cc.filter(|cc| !cc.eq_ignore_ascii_case(name)) {
        return Some(format!("{name}, {cc}"));
    }

    Some(name.to_string())
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

pub(crate) fn preview_route_points(route_points: &[ActivityRoutePoint]) -> Vec<ActivityRoutePoint> {
    if route_points.len() <= MAX_PREVIEW_POINTS {
        return route_points.to_vec();
    }

    let last_index = route_points.len().saturating_sub(1);
    let mut selected_indexes = BTreeSet::from([
        0,
        last_index,
        route_points
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.latitude.total_cmp(&right.latitude))
            .map(|(index, _)| index)
            .unwrap_or(0),
        route_points
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.latitude.total_cmp(&right.latitude))
            .map(|(index, _)| index)
            .unwrap_or(last_index),
        route_points
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| left.longitude.total_cmp(&right.longitude))
            .map(|(index, _)| index)
            .unwrap_or(0),
        route_points
            .iter()
            .enumerate()
            .max_by(|(_, left), (_, right)| left.longitude.total_cmp(&right.longitude))
            .map(|(index, _)| index)
            .unwrap_or(last_index),
    ]);

    for sample_index in 0..MAX_PREVIEW_POINTS {
        if selected_indexes.len() >= MAX_PREVIEW_POINTS {
            break;
        }

        let point_index = sample_index * last_index / (MAX_PREVIEW_POINTS.saturating_sub(1));
        selected_indexes.insert(point_index);
    }

    if selected_indexes.len() < MAX_PREVIEW_POINTS {
        for point_index in 0..route_points.len() {
            if selected_indexes.len() >= MAX_PREVIEW_POINTS {
                break;
            }

            selected_indexes.insert(point_index);
        }
    }

    selected_indexes
        .into_iter()
        .take(MAX_PREVIEW_POINTS)
        .map(|index| route_points[index].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support as fixtures;

    fn route_points() -> Vec<ActivityRoutePoint> {
        let parsed = crate::activity_parser::parse_activity_data(
            "unmarked_01.fit",
            "fit",
            fixtures::recordings::UNMARKED_01_FIT,
        )
        .unwrap();
        parsed.derived_data.route_points
    }

    #[test]
    fn preview_keeps_endpoints_and_coordinate_extrema_in_recording_order() {
        let mut route = route_points();
        route.truncate(40);
        assert_eq!(route.len(), 40);
        let latitudes = route.iter().map(|point| point.latitude).collect::<Vec<_>>();
        let longitudes = route.iter().map(|point| point.longitude).collect::<Vec<_>>();
        route[7].latitude = latitudes.iter().copied().reduce(f64::max).unwrap() + 0.01;
        route[11].latitude = latitudes.iter().copied().reduce(f64::min).unwrap() - 0.01;
        route[13].longitude = longitudes.iter().copied().reduce(f64::min).unwrap() - 0.01;
        route[29].longitude = longitudes.iter().copied().reduce(f64::max).unwrap() + 0.01;
        let selected = preview_route_points(&route);
        assert_eq!(selected.len(), MAX_PREVIEW_POINTS);
        assert_eq!(selected.first(), route.first());
        assert_eq!(selected.last(), route.last());
        for index in [7, 11, 13, 29] {
            assert!(selected.contains(&route[index]));
        }
        let indexes = selected
            .iter()
            .map(|point| route.iter().position(|source| source == point).unwrap())
            .collect::<Vec<_>>();
        assert!(indexes.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn short_and_empty_previews_keep_exact_recorded_coordinates() {
        let mut route = route_points();
        route.truncate(3);
        assert_eq!(preview_route_points(&route), route);
        assert!(preview_route_points(&[]).is_empty());
        assert_eq!(ActivityCardPreviewPoint::from(&route[0]).latitude, route[0].latitude);
    }

    #[tokio::test]
    async fn empty_recording_saves_an_empty_card_and_a_retry_reuses_its_keys() {
        for _ in 0..2 {
            let activity = fixtures::activity();
            let db = fixtures::postgres_mock()
                .append_query_results([vec![activity.clone()], vec![activity]])
                .append_exec_results([fixtures::written(), fixtures::written()])
                .into_connection();
            rebuild(&db, fixtures::activity_task()).await.unwrap();
            let log = format!("{:?}", db.into_transaction_log());
            assert!(log.contains("activity_cards"));
            assert!(log.contains("ON CONFLICT"));
            assert!(log.contains("DELETE"));
            assert!(!log.contains(r#"INSERT INTO \"activity_card_preview_points\""#));
            assert!(log.contains("COMMIT"));
        }
    }

    #[tokio::test]
    async fn saved_preview_preserves_manual_location_and_every_supported_sport() {
        for sport in ["ride", "hike"] {
            let mut route = route_points();
            route.truncate(4);
            let recording = crate::activity_data::ActivityDerivedData {
                route_points: route,
                ..Default::default()
            };
            let stored = crate::activity_data::serialize_derived_activity_data(&recording).with_source_metadata(
                crate::activity_data::ActivitySourceMetadata {
                    location: Some("Saved location".into()),
                    ..Default::default()
                },
            );
            let activity = activities::Model {
                sport: sport.into(),
                derived_data_json: Some(stored),
                ..fixtures::activity()
            };
            let db = fixtures::postgres_mock()
                .append_query_results([vec![activity.clone()], vec![activity]])
                .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()])
                .into_connection();
            rebuild(&db, fixtures::activity_task()).await.unwrap();
            let log = db.into_transaction_log();
            let sql = format!("{log:?}");
            assert!(sql.contains("Saved location"));
            assert!(sql.contains("activity_card_preview_points"));
            assert!(sql.contains("COMMIT"));
        }
    }

    #[tokio::test]
    async fn a_changed_or_deleted_source_cannot_publish_stale_card_output() {
        for deleted in [false, true] {
            let source = fixtures::activity();
            let changed = activities::Model {
                derived_data_json: Some(Default::default()),
                ..source.clone()
            };
            let current = if deleted { vec![] } else { vec![changed] };
            let db = fixtures::postgres_mock()
                .append_query_results([vec![source], current])
                .into_connection();
            let result = rebuild(&db, fixtures::activity_task()).await;
            assert_eq!(result.is_ok(), deleted);
            let sql = format!("{:?}", db.into_transaction_log());
            assert!(!sql.contains("INSERT"));
            assert!(sql.contains("ROLLBACK"));
        }
    }

    #[tokio::test]
    async fn an_unowned_or_missing_activity_produces_no_card() {
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<activities::Model>::new()])
            .into_connection();
        rebuild(&db, fixtures::activity_task()).await.unwrap();
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(log[0].statements()[0].sql.contains("user_id"));
    }
    #[test]
    fn formats_city_and_admin1_when_available() {
        let result = format_record(&Record {
            lat: 0.0,
            lon: 0.0,
            name: "Traverse City".to_string(),
            admin1: "MI".to_string(),
            admin2: "Grand Traverse".to_string(),
            cc: "US".to_string(),
        });

        assert_eq!(result.as_deref(), Some("Traverse City, MI"));
    }

    #[test]
    fn falls_back_to_city_and_country_code_when_admin1_is_missing() {
        let result = format_record(&Record {
            lat: 0.0,
            lon: 0.0,
            name: "Reykjavik".to_string(),
            admin1: "".to_string(),
            admin2: "".to_string(),
            cc: "IS".to_string(),
        });

        assert_eq!(result.as_deref(), Some("Reykjavik, IS"));
    }

    #[test]
    fn reverse_geocodes_route_points_to_human_location() {
        let result = location_from_route_points(&[ActivityRoutePoint {
            elapsed_seconds: 0,
            latitude: 45.523,
            longitude: -122.676,
            distance_meters: None,
            elevation_meters: None,
            speed_mps: None,
            heart_rate_bpm: None,
            cadence_rpm: None,
            power_watts: None,
        }]);

        assert!(result.is_some());
        assert!(!result.as_deref().unwrap_or_default().contains("122.676"));
    }
}
