use super::test_support::local_user_json as history;
use bike_core::entities::integration_events;
use bike_core::test_support as fixtures;
use sea_orm::DbErr;

#[tokio::test]
async fn account_history_scopes_roots_and_preserves_temporal_context() {
    let event = fixtures::integration_event();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .append_query_results([vec![event.clone()]])
        .into_connection();
    let rows = history(db.clone(), "/api/integration-events/strava", 200).await;
    assert_eq!(rows[0]["id"], event.id);
    assert_eq!(rows[0]["payload"], event.payload.unwrap());
    let log = db.into_transaction_log();
    let query = &log.last().unwrap().statements()[0];
    for clause in [
        "\"provider\" = $1",
        "\"user_id\" = $2",
        "\"event_type\" = $3",
        "->> $4) IS NULL",
        "\"created_at\" DESC",
        "\"id\" DESC",
        "LIMIT $5",
    ] {
        assert!(query.sql.contains(clause), "{query:?}");
    }
    assert_eq!(
        query.values.as_ref().unwrap().0,
        vec![
            "strava".into(),
            fixtures::customer().id.into(),
            "strava.event.accepted".into(),
            "parent_event_id".into(),
            25_u64.into()
        ]
    );
}

#[tokio::test]
async fn account_history_without_events_returns_an_empty_list() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .append_query_results([Vec::<integration_events::Model>::new()])
        .into_connection();
    assert_eq!(
        history(db, "/api/integration-events/strava", 200).await,
        serde_json::json!([])
    );
}

#[tokio::test]
async fn failed_history_read_is_an_error_instead_of_empty_success() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .append_query_errors([DbErr::Custom("fixture history unavailable".into())])
        .into_connection();
    history(db, "/api/integration-events/strava", 500).await;
}

#[tokio::test]
async fn admin_history_applies_provider_owner_activity_import_and_limit_filters() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![bike_core::auth::entities::users::Model {
            is_admin: Some(true),
            ..fixtures::customer()
        }]])
        .append_query_results([vec![fixtures::integration_event()]])
        .into_connection();
    let rows = history(
        db.clone(),
        "/api/admin/integration-events?provider=strava&user_id=7&activity_id=42&import_id=18&limit=999",
        200,
    )
    .await;
    assert_eq!(rows.as_array().unwrap().len(), 1);
    let log = db.into_transaction_log();
    let query = &log.last().unwrap().statements()[0];
    assert!(query.sql.contains("->> $3"), "{query:?}");
    assert!(query.sql.contains("->> $5"), "{query:?}");
    assert_eq!(
        query.values.as_ref().unwrap().0,
        vec![
            "strava".into(),
            7.into(),
            "activity_id".into(),
            "42".into(),
            "import_id".into(),
            "18".into(),
            200_u64.into()
        ]
    );
}
