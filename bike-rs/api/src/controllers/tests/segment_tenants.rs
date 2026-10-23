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
