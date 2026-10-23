use crate::controllers::test_support::user_request;
use crate::storage::AppStorage;
use axum::{body::Body, http::Request};
use bike_core::auth::entities::users;
use bike_core::test_support as fixtures;
use sea_orm::DatabaseConnection;
use serde_json::Value;

async fn request(db: &DatabaseConnection, reader: &users::Model, suffix: &str) -> Value {
    user_request(
        AppStorage::for_test(db.clone()),
        reader.pid,
        Request::builder()
            .uri(format!("/api/segments/1/{suffix}"))
            .body(Body::empty())
            .unwrap(),
        200,
    )
    .await
}

async fn personal_results(reader: users::Model) -> Vec<i64> {
    let scenario = fixtures::shared_segments();
    let db = scenario
        .personal_read_mock(&reader, &scenario.segment)
        .into_connection();
    let response = request(&db, &reader, "personal-achievements").await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[2].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.segment.id.into(), reader.id.into()],
        "personal results must select the authenticated rider and requested segment"
    );
    response
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["effort_id"].as_i64().unwrap())
        .collect()
}

async fn comparison_results(reader: users::Model) -> Vec<i64> {
    let scenario = fixtures::shared_segments();
    let db = scenario
        .comparison_read_mock(&reader, &scenario.segment)
        .into_connection();
    let response = request(&db, &reader, "comparison").await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[2].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.segment.id.into(), reader.id.into()],
        "comparison efforts must select the authenticated rider and requested segment"
    );
    response
        .get("efforts")
        .map(|rows| {
            rows.as_array()
                .unwrap()
                .iter()
                .map(|row| row["id"].as_i64().unwrap())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn creator_sees_only_their_own_personal_positions() {
    assert_eq!(personal_results(fixtures::shared_segments().creator).await, vec![1, 2]);
}

#[tokio::test]
async fn contributor_sees_their_positions_on_another_creators_segment() {
    assert_eq!(personal_results(fixtures::shared_segments().contributor).await, vec![3]);
}

#[tokio::test]
async fn spectator_has_no_personal_positions_despite_other_athletes_results() {
    assert!(personal_results(fixtures::shared_segments().spectator).await.is_empty());
}

#[tokio::test]
async fn personal_access_checks_segment_existence_without_requiring_its_creator() {
    let scenario = fixtures::shared_segments();
    let db = scenario
        .personal_read_mock(&scenario.contributor, &scenario.segment)
        .into_connection();
    request(&db, &scenario.contributor, "personal-achievements").await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[1].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.segment.id.into(), 1_u64.into()]
    );
}

#[tokio::test]
async fn a_caller_cannot_select_another_athletes_personal_positions() {
    let scenario = fixtures::shared_segments();
    let db = scenario
        .personal_read_mock(&scenario.creator, &scenario.segment)
        .into_connection();
    let response = request(&db, &scenario.creator, "personal-achievements?user_id=8").await;
    assert_eq!(response[0]["effort_id"], 1);
    let log = db.into_transaction_log();
    assert_eq!(
        log[2].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.segment.id.into(), scenario.creator.id.into()]
    );
}

#[tokio::test]
async fn creator_comparison_excludes_the_contributors_effort() {
    assert_eq!(
        comparison_results(fixtures::shared_segments().creator).await,
        vec![1, 2]
    );
}

#[tokio::test]
async fn contributor_comparison_excludes_the_creators_efforts() {
    assert_eq!(
        comparison_results(fixtures::shared_segments().contributor).await,
        vec![3]
    );
}

#[tokio::test]
async fn spectator_comparison_does_not_fall_back_to_other_athletes_efforts() {
    assert!(comparison_results(fixtures::shared_segments().spectator)
        .await
        .is_empty());
}

#[tokio::test]
async fn comparison_activity_hydration_is_scoped_to_the_authenticated_rider() {
    let scenario = fixtures::shared_segments();
    let db = scenario
        .comparison_read_mock(&scenario.creator, &scenario.segment)
        .into_connection();
    request(&db, &scenario.creator, "comparison").await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[3].statements()[0].values.as_ref().unwrap().0,
        vec![
            scenario.creator.id.into(),
            scenario.activities[0].id.into(),
            scenario.activities[0].id.into()
        ]
    );
}

#[tokio::test]
async fn yearly_best_activity_hydration_is_scoped_to_the_authenticated_rider() {
    let scenario = fixtures::shared_segments();
    let efforts: Vec<_> = scenario
        .efforts_for(scenario.segment.id)
        .into_iter()
        .filter(|row| row.user_id == scenario.creator.id)
        .collect();
    let db = fixtures::segment_access_mock(&scenario.creator, &scenario.segment)
        .append_query_results([efforts])
        .append_query_results([vec![scenario.activities[0].clone()]])
        .into_connection();
    request(&db, &scenario.creator, "yearly-bests").await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[3].statements()[0].values.as_ref().unwrap().0,
        vec![
            scenario.creator.id.into(),
            scenario.activities[0].id.into(),
            scenario.activities[0].id.into()
        ]
    );
}

async fn browse(reader: users::Model, detail: bool) -> (Value, DatabaseConnection) {
    let mut scenario = fixtures::shared_segments();
    scenario.segment.original_filename = Some("private-ride.gpx".into());
    scenario.segment.starred = true;
    scenario.segment.source_activity_id = Some(scenario.activities[0].id);
    scenario.segment.source_start_route_point_index = Some(2);
    scenario.segment.source_end_route_point_index = Some(5);
    let db = if detail {
        scenario.detail_read_mock(&reader, &scenario.segment)
    } else {
        scenario.catalog_read_mock(&reader)
    }
    .into_connection();
    let response = user_request(
        AppStorage::for_test(db.clone()),
        reader.pid,
        Request::builder()
            .uri(if detail { "/api/segments/1" } else { "/api/segments" })
            .body(Body::empty())
            .unwrap(),
        200,
    )
    .await;
    (response, db)
}

async fn catalog_ids(reader: users::Model) -> Vec<i64> {
    let (response, db) = browse(reader, false).await;
    let log = db.into_transaction_log();
    assert!(
        log[1].statements()[0]
            .values
            .as_ref()
            .is_none_or(|values| values.0.is_empty()),
        "the shared catalog must not filter by creator"
    );
    let mut ids: Vec<_> = response
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_i64().unwrap())
        .collect();
    ids.sort_unstable();
    ids
}

#[tokio::test]
async fn creator_can_discover_both_global_segments() {
    assert_eq!(catalog_ids(fixtures::shared_segments().creator).await, vec![1, 2]);
}

#[tokio::test]
async fn contributor_can_discover_segments_created_by_another_rider() {
    assert_eq!(catalog_ids(fixtures::shared_segments().contributor).await, vec![1, 2]);
}

#[tokio::test]
async fn spectator_can_discover_segments_without_any_efforts() {
    assert_eq!(catalog_ids(fixtures::shared_segments().spectator).await, vec![1, 2]);
}

#[tokio::test]
async fn creator_can_manage_their_shared_segment() {
    let (response, _) = browse(fixtures::shared_segments().creator, true).await;
    assert_eq!(response["can_manage"], true);
}

#[tokio::test]
async fn contributor_opens_shared_segment_with_their_own_pr() {
    let (response, db) = browse(fixtures::shared_segments().contributor, true).await;
    assert_eq!(response["current_user_pr_duration_seconds"], 300);
    let log = db.into_transaction_log();
    assert_eq!(
        log[1].statements()[0].values.as_ref().unwrap().0,
        vec![1.into(), 1_u64.into()]
    );
    assert_eq!(
        log[3].statements()[0].values.as_ref().unwrap().0,
        vec![8.into(), 1.into()]
    );
}

#[tokio::test]
async fn spectator_opens_shared_segment_without_borrowing_another_riders_pr() {
    let (response, _) = browse(fixtures::shared_segments().spectator, true).await;
    assert_eq!(response["current_user_pr_duration_seconds"], Value::Null);
}

#[tokio::test]
async fn noncreator_detail_excludes_creator_private_metadata_and_management() {
    let (response, _) = browse(fixtures::shared_segments().contributor, true).await;
    assert_eq!(response["can_manage"], false);
    assert_eq!(response["original_filename"], Value::Null);
    assert!(response.get("builder_source").is_none());
    assert_eq!(response["starred"], false);
}

#[tokio::test]
async fn noncreator_catalog_excludes_creator_private_metadata_and_management() {
    let (response, _) = browse(fixtures::shared_segments().spectator, false).await;
    let segment = response.as_array().unwrap().iter().find(|row| row["id"] == 1).unwrap();
    assert_eq!(segment["can_manage"], false);
    assert_eq!(segment["original_filename"], Value::Null);
    assert!(segment.get("builder_source").is_none());
    assert_eq!(segment["starred"], false);
}

#[tokio::test]
async fn creator_detail_retains_their_private_source_reference() {
    let (response, _) = browse(fixtures::shared_segments().creator, true).await;
    assert_eq!(response["builder_source"]["activity_id"], 42);
    assert_eq!(response["original_filename"], "private-ride.gpx");
}

async fn reject_foreign_write(reader: users::Model, method: &str) {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![reader.clone()]])
        .append_query_results([Vec::<bike_core::entities::segments::Model>::new()])
        .into_connection();
    user_request(
        AppStorage::for_test(db.clone()),
        reader.pid,
        Request::builder()
            .method(method)
            .uri("/api/segments/1")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"title":"Renamed by another rider"}"#))
            .unwrap(),
        404,
    )
    .await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[1].statements()[0].values.as_ref().unwrap().0,
        vec![1.into(), reader.id.into(), 1_u64.into()]
    );
}

#[tokio::test]
async fn contributor_cannot_update_another_creators_shared_segment() {
    reject_foreign_write(fixtures::shared_segments().contributor, "PUT").await;
}

#[tokio::test]
async fn spectator_cannot_delete_a_shared_segment() {
    reject_foreign_write(fixtures::shared_segments().spectator, "DELETE").await;
}

#[tokio::test]
async fn public_segment_geometry_does_not_expose_creator_recording_telemetry() {
    let mut scenario = fixtures::shared_segments();
    scenario.segment.route_data_json = Some(
        serde_json::from_value(serde_json::json!([
            {"elapsed_seconds": 0, "latitude": 42.0, "longitude": -83.0, "heart_rate_bpm": 150, "speed_mps": 7.0},
            {"elapsed_seconds": 20, "latitude": 42.01, "longitude": -83.01, "heart_rate_bpm": 160, "speed_mps": 8.0}
        ]))
        .unwrap(),
    );
    let db = scenario
        .comparison_read_mock(&scenario.spectator, &scenario.segment)
        .into_connection();
    let response = request(&db, &scenario.spectator, "comparison").await;
    assert_eq!(response["route_points"][0]["latitude"], 42.0);
    for point in response["route_points"].as_array().unwrap() {
        assert_eq!(point["heart_rate_bpm"], Value::Null);
        assert_eq!(point["speed_mps"], Value::Null);
    }
}

#[tokio::test]
async fn contributor_can_read_their_yearly_bests_on_another_creators_segment() {
    let scenario = fixtures::shared_segments();
    let db = scenario
        .comparison_read_mock(&scenario.contributor, &scenario.segment)
        .into_connection();
    let response = request(&db, &scenario.contributor, "yearly-bests").await;
    assert_eq!(response["years"][0]["effort_id"], 3);
    let log = db.into_transaction_log();
    assert_eq!(
        log[2].statements()[0].values.as_ref().unwrap().0,
        vec![1.into(), 8.into()]
    );
}
