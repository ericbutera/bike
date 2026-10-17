use super::{
    geometry,
    projection::{PendingProjection, Projection},
};
use crate::activity_data::deserialize_derived_activity_data;
use sea_orm::{DatabaseConnection, DbErr};

#[tracing::instrument(name = "heatmap.rebuild", skip_all, fields(bike.operation = "heatmap.rebuild", activity_id = pending.activity_id, generation = pending.generation))]
pub async fn prepare_activity(db: &DatabaseConnection, pending: PendingProjection) -> Result<(), DbErr> {
    let Some(source) = Projection::source(db, &pending).await? else {
        return Ok(());
    };
    if !crate::activity_sport::is_bike_activity_sport(&source.sport) || source.format.as_deref() == Some("unavailable")
    {
        Projection::publish(db, &pending, &[]).await?;
        return Ok(());
    }
    let mut derived = deserialize_derived_activity_data(source.derived_data_json.as_ref());
    derived
        .recording
        .merge(crate::activity_recording::RecordingContext::legacy(
            &source.sport,
            &source.source,
            &source.title,
        ));
    let recovered = crate::activity_recording_recovery::resolve_recording(
        db,
        pending.activity_id,
        source.user_id,
        source.started_at,
        &derived,
    )
    .await?;
    if recovered != derived.recording {
        derived.recording = recovered;
        // Persist evidence with a source-version guard. Native Activity retry reloads
        // the new generation; the old generation must not publish stale geometry.
        crate::entities::activities::Model::store_recording(
            db,
            pending.activity_id,
            source.user_id,
            source.updated_at,
            &derived,
        )
        .await?;
        return Err(DbErr::Custom(
            "Heatmap recording evidence changed; retry the current generation".into(),
        ));
    }
    let chunks = tokio::task::spawn_blocking(crate::observability::blocking_with_current_trace(move || {
        prepare_route(&derived)
    }))
    .await
    .map_err(|error| DbErr::Custom(error.to_string()))?;
    Projection::publish(db, &pending, &chunks).await?;
    Ok(())
}

#[tracing::instrument(name = "heatmap.geometry", skip_all, fields(bike.operation = "heatmap.geometry", route_point_count = derived.route_points.len() as i64))]
pub(crate) fn prepare_route(derived: &crate::activity_data::ActivityDerivedData) -> Vec<geometry::Chunk> {
    if derived.recording.excluded() {
        Vec::new()
    } else {
        geometry::prepare(&derived.route_points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{activity_details::derive_activity_detail_data, activity_recording::RecordingContext};

    #[tokio::test]
    async fn recovered_evidence_requires_native_retry_and_the_fresh_generation_can_publish() {
        use crate::{entities::activities, test_support as fixtures};
        use sea_orm::{DbBackend, MockDatabase, MockExecResult, Value};
        use std::collections::BTreeMap;
        let mut derived = derive_activity_detail_data(
            "outdoor.fit",
            "fit",
            include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
        )
        .unwrap();
        assert!(derived.route_points.len() >= 8);
        let source = activities::Model {
            derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(&derived)),
            ..fixtures::activity()
        };
        let counterpart = activities::Model {
            id: 43,
            title: "Zwift ride".into(),
            ..source.clone()
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![source.clone()], vec![counterpart]])
            .append_exec_results([MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }])
            .into_connection();
        let error = prepare_activity(
            &db,
            PendingProjection {
                activity_id: source.id,
                generation: 3,
            },
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("retry the current generation"));
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("UPDATE"));
        assert!(!log.contains("heatmap_chunks"));

        derived.recording.observe("fit.creator.manufacturer", "zwift");
        let fresh_source = activities::Model {
            derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(&derived)),
            ..source
        };
        let owner = BTreeMap::from([("user_id", Value::from(fresh_source.user_id))]);
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![fresh_source]])
            .append_query_results([vec![owner]])
            .append_exec_results((0..3).map(|_| MockExecResult {
                rows_affected: 1,
                last_insert_id: 0,
            }))
            .into_connection();
        prepare_activity(
            &db,
            PendingProjection {
                activity_id: 42,
                generation: 4,
            },
        )
        .await
        .unwrap();
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("skipped"));
        assert!(log.contains("COMMIT"));
    }

    #[tokio::test]
    async fn non_cycling_gps_routes_are_skipped_and_never_publish_heatmap_chunks() {
        use crate::{entities::activities, test_support as fixtures};
        use sea_orm::Value;
        use std::collections::BTreeMap;
        let derived = fixtures::parsed_reprocess_activity().derived_data;
        assert!(!geometry::prepare(&derived.route_points).is_empty());
        for sport in ["hike", "walk", "run", "swim", "workout", "pickleball"] {
            let activity = activities::Model {
                sport: sport.into(),
                derived_data_json: Some(crate::activity_data::serialize_derived_activity_data(&derived)),
                ..fixtures::activity()
            };
            let owner = BTreeMap::from([("user_id", Value::from(activity.user_id))]);
            let pending = PendingProjection {
                activity_id: activity.id,
                generation: fixtures::heatmap_projection().generation,
            };
            let db = fixtures::postgres_mock()
                .append_query_results([vec![activity]])
                .append_query_results([vec![owner]])
                .append_exec_results((0..4).map(|_| fixtures::written()))
                .into_connection();
            prepare_activity(&db, pending).await.unwrap();
            let log = format!("{:?}", db.into_transaction_log());
            assert!(log.contains("skipped"), "{sport}: {log}");
            assert!(log.contains("DELETE"));
            assert!(log.contains("COMMIT"));
            assert!(!log.contains("INSERT"));
        }
    }

    #[test]
    fn legacy_virtual_and_zwift_flags_remove_chunks_while_real_rides_keep_them() {
        for (sport, source, title, excluded) in [
            ("VirtualRide", "strava_sync", "Morning ride", true),
            ("road_ride", "strava_sync", "Zwift morning ride", true),
            ("indoor trainer ride", "manual_upload", "Morning ride", true),
            ("road_ride", "strava_sync", "Morning ride", false),
        ] {
            let mut derived = derive_activity_detail_data(
                "outdoor.fit",
                "fit",
                include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
            )
            .unwrap();
            derived.recording.merge(RecordingContext::legacy(sport, source, title));
            assert_eq!(prepare_route(&derived).is_empty(), excluded);
        }
    }
}

#[cfg(test)]
mod recording_tests {
    use super::*;
    use crate::activity_details::derive_activity_detail_data;

    #[test]
    fn real_routes_in_multiple_countries_contribute_without_geography_classification() {
        let original = derive_activity_detail_data(
            "outdoor.fit",
            "fit",
            include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
        )
        .unwrap();
        for (latitude, longitude) in [(45.0, -120.0), (52.0, 0.0), (35.0, 140.0)] {
            let mut derived = original.clone();
            for (index, point) in derived.route_points.iter_mut().enumerate() {
                point.latitude = latitude;
                point.longitude = longitude + index as f64 * 0.0002;
            }
            assert!(!prepare_route(&derived).is_empty());
            derived.recording.observe("fit.creator.manufacturer", "zwift");
            assert!(prepare_route(&derived).is_empty());
        }
    }
}
