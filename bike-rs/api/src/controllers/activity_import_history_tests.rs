use bike_core::test_support as fixtures;
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn admin_import_route_keeps_domain_metadata_after_graph_retirement() {
    let imported = fixtures::activity_import();
    let db = fixtures::postgres_mock()
        .append_query_results([vec![bike_core::auth::entities::users::Model {
            is_admin: Some(true),
            ..fixtures::customer()
        }]])
        .append_query_results([vec![imported.clone()]])
        .into_connection();
    let payload = crate::controllers::test_support::local_user_json(
        db.clone(),
        &format!("/api/admin/activity-imports/{}", imported.id),
        200,
    )
    .await;
    assert_eq!(payload["id"], imported.id);
    assert_eq!(payload["activity_id"], serde_json::json!(imported.activity_id));
    assert_eq!(payload["original_filename"], imported.original_filename);
    for retired in ["graph", "nodes", "attempts", "storage_path"] {
        assert!(payload.get(retired).is_none());
    }
    assert_eq!(db.into_transaction_log().len(), 2);
}

#[tokio::test]
async fn import_detail_route_reports_missing_import_after_admin_verification() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![bike_core::auth::entities::users::Model {
            is_admin: Some(true),
            ..fixtures::customer()
        }]])
        .append_query_results([Vec::<bike_core::entities::activity_imports::Model>::new()])
        .into_connection();
    crate::controllers::test_support::local_user_json(db, "/api/admin/activity-imports/17", 404).await;
}

#[tokio::test]
async fn import_detail_route_rejects_non_admin_before_reading_import_metadata() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::customer()]])
        .into_connection();
    crate::controllers::test_support::local_user_json(db.clone(), "/api/admin/activity-imports/17", 403).await;
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn import_detail_returns_domain_metadata_without_execution_or_storage_payloads() {
    let imported = fixtures::activity_import();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![imported.clone()]])
        .into_connection();

    let response = crate::activity_import_history::detail(&db, imported.id).await.unwrap();

    assert_eq!(response.id, imported.id);
    assert_eq!(response.activity_id, imported.activity_id);
    assert_eq!(response.status, imported.status);
    assert_eq!(response.original_filename, imported.original_filename);
    let payload = serde_json::to_value(response).unwrap();
    for removed in ["graph", "nodes", "attempts", "storage_path"] {
        assert!(payload.get(removed).is_none());
    }
}

#[tokio::test]
async fn import_detail_reports_a_missing_import() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<bike_core::entities::activity_imports::Model>::new()])
        .into_connection();
    let error = crate::activity_import_history::detail(&db, 17).await.unwrap_err();
    assert_eq!(error.message, "Activity import not found");
}
