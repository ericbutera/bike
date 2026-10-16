use super::*;
use crate::{
    activity_details::{derive_activity_detail_data, serialize_derived_activity_data},
    background_jobs::{
        batches,
        entities::{pipeline_outputs, worker_batches},
    },
    entities::{activity_analytics, segment_summaries, segment_user_summaries},
    segment_support::serialize_segment_route_points,
    test_support,
};
use sea_orm::{ConnectionTrait, Database, EntityTrait, Schema, Set};

async fn fixture() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        schema.create_table_from_entity(activities::Entity),
        schema.create_table_from_entity(segments::Entity),
        schema.create_table_from_entity(segment_efforts::Entity),
        schema.create_table_from_entity(activity_analytics::Entity),
        schema.create_table_from_entity(segment_summaries::Entity),
        schema.create_table_from_entity(segment_user_summaries::Entity),
        schema.create_table_from_entity(pipeline_outputs::Entity),
        schema.create_table_from_entity(worker_batches::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    let derived = derive_activity_detail_data(
        "outdoor.fit",
        "fit",
        include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
    )
    .unwrap();
    let activity = activities::Model {
        derived_data_json: Some(serialize_derived_activity_data(&derived).unwrap()),
        ..test_support::activity()
    };
    let segment = segments::Model {
        route_data_json: Some(serialize_segment_route_points(&derived.route_points)),
        ..test_support::segment()
    };
    test_support::insert(&db, activities::ActiveModel::from(activity)).await;
    test_support::insert(&db, segments::ActiveModel::from(segment)).await;
    db
}

#[tokio::test]
async fn standalone_matching_publishes_efforts_and_both_analytics_outputs() {
    let db = fixture().await;
    process_activity_segment_regeneration(&db, 42)
        .await
        .unwrap();
    assert_eq!(
        segment_efforts::Model::count_for_activity(&db, 42)
            .await
            .unwrap(),
        1
    );
    let summary = segment_summaries::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.effort_count, 1);
    let analytics = activity_analytics::Entity::find_by_id(42)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(analytics.segment_effort_count, 1);
    assert_eq!(analytics.kom_count, 1);
}

#[tokio::test]
async fn standalone_rematching_clears_analytics_when_an_activity_no_longer_matches() {
    let db = fixture().await;
    process_activity_segment_regeneration(&db, 42)
        .await
        .unwrap();
    test_support::update(
        &db,
        activities::ActiveModel {
            id: Set(42),
            derived_data_json: Set(None),
            ..Default::default()
        },
    )
    .await;
    process_activity_segment_regeneration(&db, 42)
        .await
        .unwrap();
    assert_eq!(
        segment_efforts::Model::count_for_activity(&db, 42)
            .await
            .unwrap(),
        0
    );
    assert!(activity_analytics::Entity::find_by_id(42)
        .one(&db)
        .await
        .unwrap()
        .is_none());
    let summary = segment_summaries::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(summary.effort_count, 0);
}

#[tokio::test]
async fn batch_matching_retains_affected_segments_for_the_parent_cache_barrier() {
    let db = fixture().await;
    test_support::insert(
        &db,
        worker_batches::ActiveModel {
            id: Set(99),
            user_id: Set(7),
            kind: Set("segments".into()),
            source: Set(serde_json::json!({"segment_ids":[],"caches_scheduled":false})),
            cursor: Set(0),
            sealed: Set(false),
            status: Set("running".into()),
            updated_at: Set(test_support::timestamp()),
        },
    )
    .await;
    batches::scope(Some(99), process_activity_segment_regeneration(&db, 42))
        .await
        .unwrap();
    let batch = worker_batches::Model::find_by_id(&db, 99)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(batch.source["segment_ids"], serde_json::json!([1]));
    assert!(segment_summaries::Entity::find_by_id(1)
        .one(&db)
        .await
        .unwrap()
        .is_none());
    let analytics = activity_analytics::Entity::find_by_id(42)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(analytics.segment_effort_count, 1);
}
