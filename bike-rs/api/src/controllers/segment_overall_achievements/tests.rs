use super::*;
use crate::controllers::test_support::{local_user_json, user_request};
use axum::{body::Body, http::Request};
use bike_core::test_support::{self as fixtures, SharedSegments};
use sea_orm::DatabaseConnection;
use serde_json::{json, Value};
use tower::ServiceExt;

const PATH: &str = "/api/segments/1/overall-achievements";

async fn read_saved(
    scenario: &SharedSegments,
    reader: &users::Model,
    segment: &segments::Model,
) -> (Value, DatabaseConnection) {
    let db = scenario.overall_read_mock(reader, segment).into_connection();
    let response = request_as(&db, reader, segment.id).await;
    (response, db)
}

async fn request_as(db: &DatabaseConnection, reader: &users::Model, segment_id: i32) -> Value {
    user_request(
        AppStorage::for_test(db.clone()),
        reader.pid,
        Request::builder()
            .uri(format!("/api/segments/{segment_id}/overall-achievements"))
            .body(Body::empty())
            .unwrap(),
        200,
    )
    .await
}

fn visible_athletes(response: &Value) -> Vec<i64> {
    response
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["user_id"].as_i64().unwrap())
        .collect()
}

#[tokio::test]
async fn segment_creator_with_efforts_can_read_both_athletes_standings() {
    let scenario = fixtures::shared_segments();
    let (response, _) = read_saved(&scenario, &scenario.creator, &scenario.segment).await;
    assert_eq!(
        visible_athletes(&response),
        vec![i64::from(scenario.creator.id), i64::from(scenario.contributor.id)]
    );
}

#[tokio::test]
async fn another_athlete_with_efforts_can_read_both_athletes_standings() {
    let scenario = fixtures::shared_segments();
    let (response, _) = read_saved(&scenario, &scenario.contributor, &scenario.segment).await;
    assert_eq!(
        visible_athletes(&response),
        vec![i64::from(scenario.creator.id), i64::from(scenario.contributor.id)]
    );
}

#[tokio::test]
async fn athlete_without_efforts_can_read_both_athletes_standings() {
    let scenario = fixtures::shared_segments();
    let (response, _) = read_saved(&scenario, &scenario.spectator, &scenario.segment).await;
    assert_eq!(
        visible_athletes(&response),
        vec![i64::from(scenario.creator.id), i64::from(scenario.contributor.id)]
    );
}

#[tokio::test]
async fn segment_access_does_not_require_the_creator() {
    let scenario = fixtures::shared_segments();
    let (_, db) = read_saved(&scenario, &scenario.spectator, &scenario.segment).await;
    // SeaORM mocks do not apply filters: this is the regression guard against restoring creator scope.
    let log = db.into_transaction_log();
    assert_eq!(
        log[1].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.segment.id.into(), 1_u64.into()]
    );
}

#[tokio::test]
async fn standings_are_requested_for_the_selected_segment() {
    let scenario = fixtures::shared_segments();
    let (_, db) = read_saved(&scenario, &scenario.spectator, &scenario.other_segment).await;
    let log = db.into_transaction_log();
    assert_eq!(
        log[2].statements()[0].values.as_ref().unwrap().0,
        vec![scenario.other_segment.id.into()]
    );
}

#[tokio::test]
async fn saved_positions_are_not_renumbered() {
    let mut scenario = fixtures::shared_segments();
    scenario.standings[1].position = 7;
    let (response, _) = read_saved(&scenario, &scenario.creator, &scenario.segment).await;
    assert_eq!(response[1]["position"], 7);
}

#[test]
fn saved_standings_are_enriched_with_profile_names() {
    let saved = fixtures::overall_achievement();
    let names = HashMap::from([(saved.user_id, "Rider name".into())]);
    let response = OverallAchievementResponse::from_saved(saved, &names);
    assert_eq!(response.rider_name.as_deref(), Some("Rider name"));
}

#[tokio::test]
async fn missing_profile_names_leave_saved_standings_visible() {
    let scenario = fixtures::shared_segments();
    let db = fixtures::segment_access_mock(&scenario.creator, &scenario.segment)
        .append_query_results([scenario.standings_for(scenario.segment.id)])
        .append_query_results([Vec::<users::Model>::new()])
        .into_connection();
    let response = request_as(&db, &scenario.creator, scenario.segment.id).await;
    assert_eq!(
        visible_athletes(&response),
        vec![i64::from(scenario.creator.id), i64::from(scenario.contributor.id)]
    );
}

#[test]
fn unavailable_profile_names_are_serialized_as_null() {
    let response = OverallAchievementResponse::from_saved(fixtures::overall_achievement(), &HashMap::new());
    assert_eq!(serde_json::to_value(response).unwrap()["rider_name"], Value::Null);
}

#[tokio::test]
async fn reads_do_not_recalculate_or_write_standings() {
    let scenario = fixtures::shared_segments();
    let (_, db) = read_saved(&scenario, &scenario.creator, &scenario.segment).await;
    let log = db.into_transaction_log();
    assert_eq!(
        log.len(),
        4,
        "authentication, segment access, saved standings and profile names only"
    );
    assert!(log
        .iter()
        .flat_map(|transaction| transaction.statements())
        .all(|statement| statement.sql.starts_with("SELECT")));
}

#[tokio::test]
async fn missing_saved_output_returns_an_empty_list() {
    let scenario = fixtures::shared_segments();
    let db = fixtures::segment_access_mock(&scenario.creator, &scenario.segment)
        .append_query_results([Vec::<segment_overall_achievements::Model>::new()])
        .into_connection();
    assert_eq!(request_as(&db, &scenario.creator, scenario.segment.id).await, json!([]));
}

#[tokio::test]
async fn missing_segments_return_not_found() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .append_query_results([Vec::<segments::Model>::new()])
        .into_connection();
    local_user_json(db, PATH, 404).await;
}

#[tokio::test]
async fn saved_output_failures_are_reported() {
    let scenario = fixtures::shared_segments();
    let db = fixtures::segment_access_mock(&scenario.creator, &scenario.segment)
        .append_query_errors([sea_orm::DbErr::Custom("fixture output unavailable".into())])
        .into_connection();
    local_user_json(db, PATH, 500).await;
}

#[tokio::test]
async fn authentication_is_required() {
    let state = AppStorage::for_test(fixtures::postgres_mock().into_connection());
    let response = crate::controllers::routes()
        .with_state(Arc::new(state))
        .oneshot(Request::builder().uri(PATH).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), 401);
}

#[tokio::test]
async fn another_segment_returns_its_own_standings() {
    let scenario = fixtures::shared_segments();
    let (response, _) = read_saved(&scenario, &scenario.spectator, &scenario.other_segment).await;
    assert_eq!(
        visible_athletes(&response),
        vec![i64::from(scenario.contributor.id), i64::from(scenario.creator.id)]
    );
}
