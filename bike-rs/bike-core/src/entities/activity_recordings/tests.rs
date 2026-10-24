use super::*;
use crate::test_support as fixtures;
use ::object_store::{path::Path, ObjectStore as _, ObjectStoreExt};
use futures::TryStreamExt;

fn activity() -> super::super::activities::Model {
    let mut data = fixtures::parsed_reprocess_activity().derived_data;
    // Repeated offsets and an absent sensor value must survive in their saved order.
    data.route_points.swap(0, 1);
    data.chart_points[0].power_watts = None;
    super::super::activities::Model {
        derived_data_json: Some((&data).into()),
        ..fixtures::activity()
    }
}

async fn saved(
    objects: &ObjectStore,
    backend: &::object_store::memory::InMemory,
    activity: &super::super::activities::Model,
) -> Model {
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<Model>::new()])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Model::store(&db, objects, activity).await.unwrap();
    let keys: Vec<_> = backend.list(None).try_collect().await.unwrap();
    let key = keys.last().unwrap().location.as_ref();
    let bytes = objects
        .for_owner(ObjectOwner::BikeTasks)
        .read_bytes(key)
        .await
        .unwrap()
        .unwrap();
    fixtures::activity_recording(&ObjectRef {
        path: key.into(),
        sha256: crate::object_store::checksum(&bytes),
        bytes: bytes.len(),
    })
}

#[tokio::test]
async fn stored_samples_reload_in_their_original_order_and_unchanged_retry_reuses_the_artifact() {
    let activity = activity();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::ActivityImports);
    let reference = saved(&objects, &backend, &activity).await;
    let db = fixtures::postgres_mock()
        .append_query_results([vec![reference.clone()], vec![reference.clone()]])
        .into_connection();
    Model::store(&db, &objects, &activity).await.unwrap();
    let recording = Model::load(&db, &objects, activity.id, activity.user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        recording,
        Recording::from(ActivityDerivedData::from(activity.derived_data_json.unwrap()))
    );
    assert_eq!(backend.list(None).try_collect::<Vec<_>>().await.unwrap().len(), 1);
    assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT"));
}

#[tokio::test]
async fn changed_samples_get_a_new_reference_without_replacing_the_previous_artifact() {
    let source = activity();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let original = saved(&objects, &backend, &source).await;
    let mut changed = ActivityDerivedData::from(source.derived_data_json.clone().unwrap());
    changed.route_points[0].power_watts = Some(321);
    let changed = super::super::activities::Model {
        derived_data_json: Some((&changed).into()),
        ..source
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![original.clone()]])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Model::store(&db, &objects, &changed).await.unwrap();
    let keys: Vec<_> = backend.list(None).try_collect().await.unwrap();
    assert_eq!(keys.len(), 2);
    objects
        .for_owner(ObjectOwner::BikeTasks)
        .read(&original.object_reference().unwrap())
        .await
        .unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ON CONFLICT"));
    assert!(log.contains("object_sha256"));
}

#[tokio::test]
async fn metadata_and_lap_edits_keep_the_recording_reference() {
    let source = activity();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let original = saved(&objects, &backend, &source).await;
    let mut changed = ActivityDerivedData::from(source.derived_data_json.clone().unwrap());
    changed.laps.clear();
    let changed = super::super::activities::Model {
        title: "Manually edited title".into(),
        derived_data_json: Some((&changed).into()),
        ..source
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![original]])
        .into_connection();
    Model::store(&db, &objects, &changed).await.unwrap();
    assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT"));
    assert_eq!(backend.list(None).try_collect::<Vec<_>>().await.unwrap().len(), 1);
}

#[tokio::test]
async fn saved_recordings_are_scoped_to_each_rider_before_loading_objects() {
    let source = activity();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let original = saved(&objects, &backend, &source).await;
    let other = super::super::activities::Model {
        id: 99,
        user_id: 9,
        ..source.clone()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<Model>::new(), Vec::<Model>::new()])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Model::store(&db, &objects, &other).await.unwrap();
    assert!(Model::load(&db, &objects, source.id, other.user_id)
        .await
        .unwrap()
        .is_none());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("user_id"));
    assert!(log.contains("Int(Some(9))"));
    assert_eq!(backend.list(None).try_collect::<Vec<_>>().await.unwrap().len(), 2);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![original]])
        .into_connection();
    assert!(Model::load(&db, &objects, source.id, other.user_id)
        .await
        .unwrap_err()
        .to_string()
        .contains("rider"));
}

#[tokio::test]
async fn corrupted_saved_artifacts_fail_integrity_verification_without_source_repair() {
    let source = activity();
    let (objects, backend) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let original = saved(&objects, &backend, &source).await;
    backend
        .put(
            &Path::parse(&original.object_path).unwrap(),
            b"corrupt".as_slice().into(),
        )
        .await
        .unwrap();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![original]])
        .into_connection();
    assert_eq!(
        Model::load(&db, &objects, source.id, source.user_id)
            .await
            .unwrap_err()
            .status
            .as_u16(),
        422
    );
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn backfill_saves_explicit_empty_recordings_and_missing_legacy_data_rolls_back() {
    let (objects, backend) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let empty = super::super::activities::Model {
        derived_data_json: Some(Default::default()),
        ..fixtures::activity()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![empty]])
        .append_query_results([Vec::<Model>::new()])
        .append_exec_results([fixtures::written()])
        .into_connection();
    Model::backfill(&db, &objects, fixtures::activity_task()).await.unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("FOR UPDATE"));
    assert!(log.contains("COMMIT"));
    assert_eq!(backend.list(None).try_collect::<Vec<_>>().await.unwrap().len(), 1);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity()]])
        .into_connection();
    assert_eq!(
        Model::backfill(&db, &objects, fixtures::activity_task())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        422
    );
    assert!(format!("{:?}", db.into_transaction_log()).contains("ROLLBACK"));
}

#[tokio::test]
async fn backfill_uses_bounded_id_pages_and_accepts_concurrent_activity_deletion() {
    let row: std::collections::BTreeMap<String, Value> =
        [("activity_id".into(), 42.into()), ("user_id".into(), 7.into())].into();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![row]])
        .into_connection();
    let tasks = Model::pending_tasks(&db, 17).await.unwrap();
    assert_eq!(tasks[0].activity_id, 42);
    assert_eq!(tasks[0].priority, Priority::Background);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ORDER BY a.id LIMIT 16"));
    assert!(log.contains("NOT EXISTS"));
    let (objects, backend) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let db = fixtures::postgres_mock()
        .append_query_results([Vec::<super::super::activities::Model>::new()])
        .into_connection();
    Model::backfill(&db, &objects, tasks[0].clone()).await.unwrap();
    assert!(backend.list(None).try_collect::<Vec<_>>().await.unwrap().is_empty());
}

#[tokio::test]
async fn invalid_saved_reference_and_format_fail_without_falling_back_to_legacy_data() {
    let (objects, _) = fixtures::object_storage(ObjectOwner::BikeTasks);
    let reference = objects
        .transform(
            "bike/tasks/recordings/7",
            &Recording {
                format_version: 99,
                context: Default::default(),
                chart_points: vec![],
                route_points: vec![],
            },
        )
        .await
        .unwrap();
    let saved = fixtures::activity_recording(&reference);
    let db = fixtures::postgres_mock()
        .append_query_results([vec![saved.clone()]])
        .into_connection();
    assert_eq!(
        Model::load(&db, &objects, 42, 7).await.unwrap_err().status.as_u16(),
        422
    );
    assert!(Model {
        object_bytes: -1,
        ..saved
    }
    .object_reference()
    .is_err());
}

#[tokio::test]
async fn native_provider_references_reload_with_the_same_normalized_encoding() {
    let data = Recording::from(fixtures::parsed_reprocess_activity().derived_data);
    let (objects, _) = fixtures::object_storage(ObjectOwner::Strava);
    let reference = objects.transform("strava/recordings/7", &data).await.unwrap();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity_recording(&reference)]])
        .into_connection();
    assert_eq!(Model::load(&db, &objects, 42, 7).await.unwrap(), Some(data));
    let db = fixtures::postgres_mock().into_connection();
    assert!(Model::store_reference(
        &db,
        &fixtures::activity(),
        &ObjectRef {
            path: "strava/recordings/9/other.json".into(),
            ..reference
        }
    )
    .await
    .is_err());
    assert!(db.into_transaction_log().is_empty());
}
