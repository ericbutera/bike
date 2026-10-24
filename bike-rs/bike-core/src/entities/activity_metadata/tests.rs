use super::*;
use crate::{
    activity_data::{ActivityGear, StoredActivityDerivedData},
    test_support as fixtures,
};

fn source() -> super::super::activities::Model {
    let mut data = fixtures::parsed_reprocess_activity().derived_data;
    let mut second = data.laps[0].clone();
    data.laps[0].lap_index = 5;
    data.laps[0].title = "Warmup".into();
    data.laps[0].max_heart_rate_bpm = Some(171);
    second.lap_index = 1;
    second.title = "Effort".into();
    second.distance_meters = Some(1234.56);
    data.laps.push(second);
    super::super::activities::Model {
        derived_data_json: Some(
            StoredActivityDerivedData::from(&data).with_source_metadata(ActivitySourceMetadata {
                description: Some("Saved description".into()),
                location: Some("Trailhead".into()),
                private_notes: Some("Private note".into()),
                gear: Some(ActivityGear {
                    id: "bike-7".into(),
                    name: Some("Trail bike".into()),
                }),
            }),
        ),
        ..fixtures::activity()
    }
}

fn metadata() -> Model {
    Model {
        activity_id: 42,
        description: Some("Saved description".into()),
        location: Some("Trailhead".into()),
        private_notes: Some("Private note".into()),
        gear_id: Some("bike-7".into()),
        gear_name: Some("Trail bike".into()),
    }
}

fn lap_row(position: i32, lap: ActivityLap) -> laps::Model {
    laps::Model {
        activity_id: 42,
        position,
        lap_index: lap.lap_index,
        title: lap.title,
        start_offset_seconds: lap.start_offset_seconds,
        duration_seconds: lap.duration_seconds,
        distance_meters: lap.distance_meters,
        elevation_gain_meters: lap.elevation_gain_meters,
        elevation_loss_meters: lap.elevation_loss_meters,
        average_speed_mps: lap.average_speed_mps,
        max_speed_mps: lap.max_speed_mps,
        average_heart_rate_bpm: lap.average_heart_rate_bpm,
        max_heart_rate_bpm: lap.max_heart_rate_bpm,
        average_cadence_rpm: lap.average_cadence_rpm,
        max_cadence_rpm: lap.max_cadence_rpm,
        calories: lap.calories,
    }
}

#[tokio::test]
async fn typed_metadata_and_every_lap_fact_are_saved_without_reordering_lap_numbers() {
    let activity = source();
    let db = fixtures::postgres_mock()
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    Model::store(&db, &activity).await.unwrap();
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(statements.len(), 3);
    let metadata = &statements[0].values.as_ref().unwrap().0;
    assert!(metadata.contains(&"Private note".into()));
    assert!(metadata.contains(&"bike-7".into()));
    let rows: Vec<_> = statements[2].values.as_ref().unwrap().0.chunks(16).collect();
    assert_eq!(&rows[0][..4], &[42.into(), 0.into(), 5.into(), "Warmup".into()]);
    assert_eq!(&rows[1][..4], &[42.into(), 1.into(), 1.into(), "Effort".into()]);
    assert!(rows[0].contains(&171.into()));
    assert!(rows[1].contains(&1234.56.into()));
}

#[tokio::test]
async fn owned_reads_return_saved_metadata_and_laps_while_other_riders_and_missing_output_stay_empty() {
    let data = crate::activity_data::deserialize_derived_activity_data(source().derived_data_json.as_ref());
    let rows: Vec<_> = data
        .laps
        .clone()
        .into_iter()
        .enumerate()
        .map(|(i, lap)| lap_row(i as i32, lap))
        .collect();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![metadata()]])
        .append_query_results([rows])
        .into_connection();
    let saved = Model::load(&db, 42, 7).await.unwrap().unwrap();
    assert_eq!(saved.metadata, metadata());
    assert_eq!(saved.laps, data.laps);
    for user_id in [7, 9] {
        let db = fixtures::postgres_mock()
            .append_query_results([Vec::<Model>::new()])
            .into_connection();
        assert!(Model::load(&db, 42, user_id).await.unwrap().is_none());
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(format!("{log:?}").contains("user_id"));
        assert!(log[0].statements()[0]
            .values
            .as_ref()
            .unwrap()
            .0
            .contains(&user_id.into()));
    }
}

#[tokio::test]
async fn cleared_metadata_saves_nulls_and_unchanged_edits_write_nothing() {
    let before = source();
    let after = super::super::activities::Model {
        derived_data_json: Some(
            before
                .derived_data_json
                .clone()
                .unwrap()
                .with_source_metadata(Default::default()),
        ),
        ..before.clone()
    };
    let db = fixtures::postgres_mock()
        .append_exec_results((0..3).map(|_| fixtures::written()))
        .into_connection();
    Model::sync_edit(&db, &before, &before).await.unwrap();
    Model::sync_edit(&db, &before, &after).await.unwrap();
    let log = db.into_transaction_log();
    assert_eq!(log.len(), 3);
    let values = &log[0].statements()[0].values.as_ref().unwrap().0;
    assert_eq!(
        values
            .iter()
            .filter(|value| **value == sea_orm::Value::String(None))
            .count(),
        5
    );
    assert!(format!("{log:?}").contains("Warmup"));
    assert!(format!("{log:?}").contains("Effort"));
}

#[tokio::test]
async fn focused_backfill_preserves_edited_values_and_rolls_back_on_a_lap_write_failure() {
    for fail in [false, true] {
        let mock = fixtures::postgres_mock()
            .append_query_results([vec![source()]])
            .append_exec_results([fixtures::written()]);
        let mock = if fail {
            mock.append_exec_errors([DbErr::Custom("lap write failed".into())])
        } else {
            mock.append_exec_results([fixtures::written(), fixtures::written()])
        };
        let db = mock.into_connection();
        let result = Model::backfill(&db, fixtures::activity_task()).await;
        assert_eq!(result.is_err(), fail);
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("FOR UPDATE"));
        assert!(log.contains("Private note"));
        assert!(log.contains(if fail { "ROLLBACK" } else { "COMMIT" }));
        for unrelated in ["activity_recordings", "segment_efforts", "activity_earned_awards"] {
            assert!(!log.contains(unrelated));
        }
    }
}

#[tokio::test]
async fn empty_laps_clear_previous_rows_and_deleted_activities_are_successful_empty_work() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity()]])
        .append_exec_results([fixtures::written(), fixtures::written()])
        .into_connection();
    Model::backfill(&db, fixtures::activity_task()).await.unwrap();
    let log = db.into_transaction_log();
    let sql: Vec<_> = log
        .iter()
        .flat_map(|txn| txn.statements())
        .map(|s| s.sql.as_str())
        .collect();
    assert!(sql.iter().any(|sql| sql.starts_with("DELETE FROM \"activity_laps\"")));
    assert!(!sql.iter().any(|sql| sql.starts_with("INSERT INTO \"activity_laps\"")));
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<super::super::activities::Model>::new()])
        .into_connection();
    Model::backfill(&db, fixtures::activity_task()).await.unwrap();
    assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT"));
}

#[tokio::test]
async fn pending_metadata_uses_bounded_activity_id_pages_and_lap_retries_replace_the_same_positions() {
    let row: std::collections::BTreeMap<String, sea_orm::Value> =
        [("activity_id".into(), 42.into()), ("user_id".into(), 7.into())].into();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![row]])
        .into_connection();
    assert_eq!(
        Model::pending_tasks(&db, 17).await.unwrap(),
        vec![ActivityTask {
            activity_id: 42,
            user_id: 7,
            priority: Priority::Background,
        }]
    );
    let db = fixtures::postgres_mock()
        .append_exec_results((0..6).map(|_| fixtures::written()))
        .into_connection();
    for _ in 0..2 {
        Model::store(&db, &source()).await.unwrap();
    }
    let log = db.into_transaction_log();
    let statements: Vec<_> = log.iter().flat_map(|txn| txn.statements()).collect();
    assert_eq!(statements[2].values, statements[5].values);
}
