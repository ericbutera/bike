use super::*;
use crate::{
    entities::{activity_recordings, segments},
    object_store::ObjectOwner,
    test_support as fixtures,
};

fn route() -> Vec<ActivityRoutePoint> {
    fixtures::parsed_reprocess_activity().derived_data.route_points
}

#[test]
fn bounds_keep_route_extents_and_expand_for_the_matching_corridor() {
    let route = route();
    let bounds = Bounds::from_route(&route).unwrap().unwrap();
    assert_eq!(bounds.min_latitude, 45.0);
    assert_eq!(bounds.max_latitude, 45.0);
    assert_eq!(bounds.min_longitude, -120.0);
    let expanded = bounds.matching_corridor();
    assert!(expanded.min_latitude < bounds.min_latitude);
    assert!(expanded.max_latitude > bounds.max_latitude);
    assert!(expanded.min_longitude < bounds.min_longitude);
    assert!(expanded.max_longitude > bounds.max_longitude);
    assert!(Bounds::from_route(&[]).unwrap().is_none());
    assert!(Bounds::from_route(&route[..1]).unwrap().is_none());
}

#[test]
fn antimeridian_and_polar_routes_keep_conservative_longitude_coverage() {
    let mut route = route();
    route[0].longitude = 179.999;
    route[1].longitude = -179.999;
    let bounds = Bounds::from_route(&route).unwrap().unwrap();
    assert_eq!((bounds.min_longitude, bounds.max_longitude), (-180.0, 180.0));
    for latitude in [-89.999, 89.999] {
        for point in &mut route {
            point.latitude = latitude;
            point.longitude = 20.0;
        }
        let bounds = Bounds::from_route(&route).unwrap().unwrap().matching_corridor();
        assert_eq!((bounds.min_longitude, bounds.max_longitude), (-180.0, 180.0));
    }
}

#[tokio::test]
async fn invalid_coordinates_fail_before_publishing_bounds() {
    for coordinate in [f64::NAN, 91.0] {
        let mut route = route();
        route[0].latitude = coordinate;
        let db = fixtures::postgres_mock().into_connection();
        assert!(segment::Model::store_route(&db, 1, &route).await.is_err());
        assert!(db.into_transaction_log().is_empty());
    }
}

#[tokio::test]
async fn activity_and_segment_producers_save_typed_extents_and_replace_empty_outputs() {
    let activity = super::super::activities::Model {
        derived_data_json: Some((&fixtures::parsed_reprocess_activity().derived_data).into()),
        ..fixtures::activity()
    };
    let db = fixtures::postgres_mock()
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    Model::store_source(&db, &activity).await.unwrap();
    segment::Model::store_route(&db, 1, &route()).await.unwrap();
    segment::Model::store_route(&db, 1, &[]).await.unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert!(statements[0].sql.contains("activity_route_bounds"));
    assert!(statements[1].sql.contains("segment_route_bounds"));
    assert_eq!(statements[0].values.as_ref().unwrap().0[1], 3_i64.into());
    assert_eq!(statements[2].values.as_ref().unwrap().0[1], 0_i64.into());
    assert!(statements[2].sql.contains("ON CONFLICT"));
}

#[tokio::test]
async fn activity_backfill_reads_the_owned_normalized_recording_and_saves_only_bounds() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let recording = activity_recordings::Recording::from(fixtures::parsed_reprocess_activity().derived_data);
    let reference = objects.transform("bike/tasks/recordings/7", &recording).await.unwrap();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![fixtures::activity_recording(&reference)]])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Model::backfill_activity(&db, &objects, fixtures::activity_task())
        .await
        .unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    let writes: Vec<_> = statements.iter().filter(|s| s.sql.starts_with("INSERT")).collect();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].sql.contains("activity_route_bounds"));
    assert_eq!(statements.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn missing_recording_output_rolls_back_without_legacy_reconstruction_and_deletion_is_empty() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let legacy = super::super::activities::Model {
        derived_data_json: Some((&fixtures::parsed_reprocess_activity().derived_data).into()),
        ..fixtures::activity()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![legacy]])
        .append_query_results([Vec::<activity_recordings::Model>::new()])
        .into_connection();
    let error = Model::backfill_activity(&db, &objects, fixtures::activity_task())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("backfill_activity_recordings"));
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("INSERT"));
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<super::super::activities::Model>::new()])
        .into_connection();
    Model::backfill_activity(&db, &objects, fixtures::activity_task())
        .await
        .unwrap();
}

#[tokio::test]
async fn segment_backfill_locks_the_fixed_definition_and_deleted_segments_write_nothing() {
    let saved = segments::Model {
        route_data_json: Some(crate::segment_support::serialize_segment_route_points(&route())),
        ..fixtures::segment()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![saved]])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Model::backfill_segment(&db, 1).await.unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("FOR UPDATE"));
    assert!(log.contains("segment_route_bounds"));
    assert!(!log.contains("segment_efforts"));
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<segments::Model>::new()])
        .into_connection();
    Model::backfill_segment(&db, 1).await.unwrap();
    assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT"));
}

#[tokio::test]
async fn bounds_backfill_pages_keep_owned_activity_ids_and_shared_segment_ids() {
    let activity: std::collections::BTreeMap<String, sea_orm::Value> =
        [("activity_id".into(), 42.into()), ("user_id".into(), 7.into())].into();
    let id: std::collections::BTreeMap<String, sea_orm::Value> = [("id".into(), 1.into())].into();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![activity]])
        .append_query_results([vec![id]])
        .into_connection();
    assert_eq!(Model::pending_activities(&db, 17).await.unwrap()[0].activity_id, 42);
    assert_eq!(Model::pending_segments(&db, 18).await.unwrap(), vec![1]);
    let log = db.into_transaction_log();
    for (txn, after_id) in log.iter().zip([17, 18]) {
        let statement = &txn.statements()[0];
        assert!(statement.sql.contains("NOT EXISTS"));
        assert_eq!(
            statement.values.as_ref().unwrap().0,
            vec![after_id.into(), 16_u64.into()]
        );
    }
}

#[tokio::test]
async fn historical_candidates_are_bounded_cycling_ids_across_riders() {
    for bounds in [None, Bounds::from_route(&route()).unwrap()] {
        let rows: Vec<std::collections::BTreeMap<String, sea_orm::Value>> =
            [42, 99].map(|id| [("id".into(), id.into())].into()).into();
        let db = fixtures::postgres_mock().append_query_results([rows]).into_connection();
        assert_eq!(
            Model::matching_activity_ids(&db, bounds, 17, 500).await.unwrap(),
            vec![42, 99]
        );
        let log = db.into_transaction_log();
        let statement = &log[0].statements()[0];
        assert!(statement.sql.contains("\"activities\".\"sport\" IN ("));
        assert!(!statement.sql.contains("derived_data_json"));
        assert!(!statement.sql.contains("user_id"));
        let values = &statement.values.as_ref().unwrap().0;
        for sport in super::super::activities::BIKE_ACTIVITY_SPORT_VALUES.iter() {
            assert!(values.contains(&(*sport).into()));
        }
        assert!(values.contains(&17.into()));
        assert!(values.contains(&16_u64.into()));
        if bounds.is_some() {
            assert!(statement.sql.contains("UNION ALL"));
            assert!(statement.sql.contains("NOT EXISTS"));
            assert!(statement.sql.contains("&&"));
        } else {
            assert!(!statement.sql.contains("activity_route_bounds"));
        }
    }
}

#[tokio::test]
async fn historical_candidate_page_limits_are_clamped_and_empty_pages_are_preserved() {
    for (limit, expected) in [(-1, 1_u64), (0, 1), (8, 8), (500, 16)] {
        let rows = Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new();
        let db = fixtures::postgres_mock().append_query_results([rows]).into_connection();
        assert!(Model::matching_activity_ids(&db, None, 17, limit)
            .await
            .unwrap()
            .is_empty());
        let log = db.into_transaction_log();
        assert_eq!(
            log[0].statements()[0].values.as_ref().unwrap().0.last(),
            Some(&expected.into())
        );
    }
}

#[tokio::test]
async fn candidate_overlap_binds_the_expanded_corridor_for_both_route_types() {
    let bounds = Bounds::from_route(&route()).unwrap().unwrap();
    let corridor = bounds.matching_corridor();
    let expected: Vec<sea_orm::Value> = [
        corridor.min_longitude,
        corridor.min_latitude,
        corridor.max_longitude,
        corridor.max_latitude,
    ]
    .map(Into::into)
    .into();
    let rows = Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new();
    let db = fixtures::postgres_mock()
        .append_query_results([rows])
        .append_query_results([Vec::<segments::Model>::new()])
        .into_connection();
    Model::matching_activity_ids(&db, Some(bounds), 17, 8).await.unwrap();
    segments::Model::matching_routes_page(&db, 17, 8, Some(bounds))
        .await
        .unwrap();
    let log = db.into_transaction_log();
    for statement in log.iter().flat_map(|txn| txn.statements()) {
        let values = &statement.values.as_ref().unwrap().0;
        assert!(values.windows(expected.len()).any(|window| window == expected));
        assert!(statement.sql.contains("UNION ALL"));
        assert!(statement.sql.contains("NOT EXISTS"));
        assert!(statement.sql.contains("\"point_count\" >= 2"));
    }
}
#[tokio::test]
async fn segment_geometry_is_loaded_only_after_the_candidate_id_page() {
    for bounds in [None, Bounds::from_route(&route()).unwrap()] {
        let db = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::segment()]])
            .into_connection();
        let result = segments::Model::matching_routes_page(&db, 17, 500, bounds)
            .await
            .unwrap();
        assert_eq!(result[0].id, fixtures::segment().id);
        let log = db.into_transaction_log();
        let statement = &log[0].statements()[0];
        assert!(statement.sql.contains("LIMIT"));
        assert!(statement.sql.contains("\"segments\".\"id\" IN (SELECT"));
        assert!(statement.values.as_ref().unwrap().0.contains(&25_u64.into()));
        if bounds.is_some() {
            assert!(statement.sql.contains("&& box("));
            assert!(statement.sql.contains("NOT EXISTS"));
        }
    }
}

#[test]
fn bounds_include_every_route_point_regardless_of_recording_order() {
    let mut route = route();
    for (point, (latitude, longitude)) in route.iter_mut().zip([(20.0, -100.0), (-10.0, -110.0), (5.0, -120.0)]) {
        point.latitude = latitude;
        point.longitude = longitude;
    }
    let expected = Bounds {
        min_latitude: -10.0,
        max_latitude: 20.0,
        min_longitude: -120.0,
        max_longitude: -100.0,
    };
    assert_eq!(Bounds::from_route(&route).unwrap(), Some(expected));
    route.reverse();
    assert_eq!(Bounds::from_route(&route).unwrap(), Some(expected));
}

#[test]
fn corridor_preserves_the_largest_match_profile_and_recording_precision_allowance() {
    let bounds = Bounds {
        min_latitude: 0.0,
        max_latitude: 0.0,
        min_longitude: 0.0,
        max_longitude: 0.0,
    };
    let corridor = bounds.matching_corridor();
    let radius_meters = crate::segment_support::candidate_corridor_meters() + 1.0;
    let latitude_margin = (radius_meters / 6_371_000.0).to_degrees();
    assert_eq!(corridor.min_latitude, -latitude_margin);
    assert_eq!(corridor.max_latitude, latitude_margin);
    assert!(corridor.min_longitude < -latitude_margin);
    assert!(corridor.max_longitude > latitude_margin);
}

#[test]
fn longitude_corridor_widens_at_high_latitudes_in_both_hemispheres() {
    let bounds = Bounds {
        min_latitude: 0.0,
        max_latitude: 0.0,
        min_longitude: 20.0,
        max_longitude: 21.0,
    };
    let equatorial = bounds.matching_corridor();
    for latitude in [-80.0, 80.0] {
        let corridor = Bounds {
            min_latitude: latitude,
            max_latitude: latitude,
            ..bounds
        }
        .matching_corridor();
        assert!(corridor.min_longitude < equatorial.min_longitude);
        assert!(corridor.max_longitude > equatorial.max_longitude);
        assert!(corridor.min_latitude >= -90.0);
        assert!(corridor.max_latitude <= 90.0);
    }
}

#[test]
fn corridors_crossing_either_side_of_the_antimeridian_keep_all_longitudes() {
    for longitude in [-179.999, 179.999] {
        let mut route = route();
        for point in &mut route {
            point.longitude = longitude;
        }
        let bounds = Bounds::from_route(&route).unwrap().unwrap();
        assert_eq!(bounds.min_longitude, longitude);
        let corridor = bounds.matching_corridor();
        assert_eq!((corridor.min_longitude, corridor.max_longitude), (-180.0, 180.0));
    }
}

#[test]
fn corridors_at_the_poles_stay_finite_and_clamp_to_earth() {
    for latitude in [-90.0, 90.0] {
        let bounds = Bounds {
            min_latitude: latitude,
            max_latitude: latitude,
            min_longitude: 0.0,
            max_longitude: 0.0,
        };
        let corridor = bounds.matching_corridor();
        assert_eq!((corridor.min_longitude, corridor.max_longitude), (-180.0, 180.0));
        assert!(corridor.min_latitude.is_finite());
        assert!(corridor.max_latitude.is_finite());
        assert!(corridor.min_latitude >= -90.0);
        assert!(corridor.max_latitude <= 90.0);
    }
}

#[test]
fn a_longitude_span_of_exactly_half_the_earth_remains_bounded() {
    let mut route = route();
    for point in &mut route {
        point.longitude = -90.0;
    }
    route[1].longitude = 90.0;
    let bounds = Bounds::from_route(&route).unwrap().unwrap();
    assert_eq!((bounds.min_longitude, bounds.max_longitude), (-90.0, 90.0));
    route[1].longitude = 90.001;
    let bounds = Bounds::from_route(&route).unwrap().unwrap();
    assert_eq!((bounds.min_longitude, bounds.max_longitude), (-180.0, 180.0));
}

#[test]
fn invalid_coordinates_on_any_route_point_are_rejected() {
    for (latitude, longitude) in [
        (f64::NAN, 0.0),
        (f64::INFINITY, 0.0),
        (-90.001, 0.0),
        (90.001, 0.0),
        (0.0, f64::NAN),
        (0.0, f64::NEG_INFINITY),
        (0.0, -180.001),
        (0.0, 180.001),
    ] {
        let mut route = route();
        let last = route.last_mut().unwrap();
        last.latitude = latitude;
        last.longitude = longitude;
        assert!(Bounds::from_route(&route).is_err());
    }
}
