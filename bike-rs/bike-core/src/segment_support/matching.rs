use super::*;
use crate::test_support as fixtures;
use sea_orm::{DatabaseConnection, MockDatabase, Transaction};
use std::collections::BTreeMap;

fn regeneration_reads(scenario: &fixtures::SharedSegments) -> MockDatabase {
    let old_ids: Vec<BTreeMap<String, sea_orm::Value>> = scenario
        .activities
        .iter()
        .map(|activity| BTreeMap::from([("activity_id".into(), activity.id.into())]))
        .collect::<Vec<_>>();
    fixtures::postgres_mock()
        .append_query_results([old_ids])
        .append_query_results([scenario.activities.to_vec()])
        .append_query_results([Vec::<activities::Model>::new()])
}

fn regeneration_mock(scenario: &fixtures::SharedSegments) -> MockDatabase {
    regeneration_reads(scenario)
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_exec_results([fixtures::written(), fixtures::written(), fixtures::written()])
}

async fn regenerate(scenario: &fixtures::SharedSegments, db: &DatabaseConnection) -> Vec<i32> {
    let route = deserialize_segment_route_points(scenario.segment.route_data_json.as_ref());
    replace_segment_efforts_for_segment(db, scenario.segment.id, &route)
        .await
        .unwrap()
}

fn effort_owners(log: &[Transaction]) -> Vec<i32> {
    log.iter()
        .flat_map(|tx| tx.statements())
        .filter(|s| s.sql.starts_with(r#"INSERT INTO "segment_efforts""#))
        .map(|s| match &s.values.as_ref().unwrap().0[0] {
            sea_orm::Value::Int(Some(id)) => *id,
            value => panic!("expected effort owner, got {value:?}"),
        })
        .collect()
}

#[tokio::test]
async fn segment_regeneration_scans_activities_across_creators() {
    let scenario = fixtures::shared_segments().with_recordings();
    let db = regeneration_mock(&scenario).into_connection();
    regenerate(&scenario, &db).await;
    let log = db.into_transaction_log();
    let query = log
        .iter()
        .flat_map(|tx| tx.statements())
        .find(|s| s.sql.starts_with("SELECT") && s.sql.contains(r#"FROM "activities""#))
        .unwrap();
    assert!(!query.sql.contains(r#""activities"."user_id" ="#));
}

#[tokio::test]
async fn shared_segment_regeneration_preserves_both_activity_owners() {
    let scenario = fixtures::shared_segments().with_recordings();
    let db = regeneration_mock(&scenario).into_connection();
    regenerate(&scenario, &db).await;
    assert_eq!(
        effort_owners(&db.into_transaction_log()),
        vec![scenario.creator.id, scenario.contributor.id]
    );
}

#[tokio::test]
async fn segment_regeneration_does_not_fabricate_spectator_efforts() {
    let scenario = fixtures::shared_segments().with_recordings();
    let db = regeneration_mock(&scenario).into_connection();
    regenerate(&scenario, &db).await;
    assert!(!effort_owners(&db.into_transaction_log()).contains(&scenario.spectator.id));
}

#[tokio::test]
async fn a_failed_shared_effort_write_rolls_back_the_entire_replacement() {
    let scenario = fixtures::shared_segments().with_recordings();
    let db = regeneration_reads(&scenario)
        .append_exec_results([fixtures::written()])
        .append_query_errors([sea_orm::DbErr::Custom("effort write failed".into())])
        .into_connection();
    let route = deserialize_segment_route_points(scenario.segment.route_data_json.as_ref());
    let result = replace_segment_efforts_for_segment(&db, scenario.segment.id, &route).await;
    assert!(result.is_err());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("COMMIT"));
}

async fn match_contributor_activity(scenario: &fixtures::SharedSegments) -> Vec<Transaction> {
    let activity = &scenario.activities[1];
    let route = deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
    let db = fixtures::postgres_mock()
        .append_query_results([vec![scenario.segment.clone(), scenario.other_segment.clone()]])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<segments::Model>::new()])
        .append_exec_results([fixtures::written()])
        .into_connection();
    replace_segment_efforts_for_activity(&db, activity.user_id, activity.id, &route)
        .await
        .unwrap();
    db.into_transaction_log()
}

#[tokio::test]
async fn a_contributors_activity_matches_a_segment_created_by_another_athlete() {
    let scenario = fixtures::shared_segments().with_recordings();
    let log = match_contributor_activity(&scenario).await;
    let insert = log
        .iter()
        .flat_map(|tx| tx.statements())
        .find(|s| s.sql.starts_with("INSERT"))
        .unwrap();
    assert_eq!(
        &insert.values.as_ref().unwrap().0[..4],
        &[
            scenario.contributor.id.into(),
            scenario.segment.id.into(),
            scenario.activities[1].id.into(),
            1.into(),
        ]
    );
}

#[tokio::test]
async fn activity_matching_only_replaces_the_contributors_own_activity_efforts() {
    let scenario = fixtures::shared_segments().with_recordings();
    let log = match_contributor_activity(&scenario).await;
    let delete = log
        .iter()
        .flat_map(|tx| tx.statements())
        .find(|s| s.sql.starts_with("DELETE"))
        .unwrap();
    assert_eq!(
        delete.values.as_ref().unwrap().0,
        vec![scenario.contributor.id.into(), scenario.activities[1].id.into()]
    );
}

#[tokio::test]
async fn activity_matching_does_not_filter_global_route_candidates_by_creator() {
    let scenario = fixtures::shared_segments().with_recordings();
    let log = match_contributor_activity(&scenario).await;
    let query = log
        .iter()
        .flat_map(|tx| tx.statements())
        .find(|s| s.sql.starts_with("SELECT"))
        .unwrap();
    assert!(!query.sql.contains(r#""segments"."user_id" ="#));
}

#[tokio::test]
async fn empty_segment_geometry_clears_matches_for_both_athletes() {
    let scenario = fixtures::shared_segments();
    let db = regeneration_mock(&scenario).into_connection();
    let affected = replace_segment_efforts_for_segment(&db, scenario.segment.id, &[])
        .await
        .unwrap();
    assert_eq!(affected, vec![scenario.activities[0].id, scenario.activities[1].id]);
    let log = db.into_transaction_log();
    let delete = log
        .iter()
        .flat_map(|tx| tx.statements())
        .find(|s| s.sql.starts_with("DELETE"))
        .unwrap();
    assert!(delete.sql.contains(r#""segment_id" ="#));
    assert_eq!(delete.values.as_ref().unwrap().0, vec![scenario.segment.id.into()]);
}

#[tokio::test]
async fn global_activity_paging_advances_past_the_last_athletes_activity() {
    let scenario = fixtures::shared_segments().with_recordings();
    let db = regeneration_mock(&scenario).into_connection();
    regenerate(&scenario, &db).await;
    let log = db.into_transaction_log();
    let query = log
        .iter()
        .flat_map(|tx| tx.statements())
        .filter(|s| s.sql.contains(r#"FROM "activities""#))
        .nth(1)
        .unwrap();
    assert!(query
        .values
        .as_ref()
        .unwrap()
        .0
        .ends_with(&[scenario.activities[1].id.into(), 16_u64.into()]));
}
