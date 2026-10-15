use super::*;
use bike_core::test_support;
use sea_orm::ActiveModelTrait;

async fn store(db: &DatabaseConnection, id: i32, owner: i32, import_id: Option<i32>) {
    let active: activities::ActiveModel = activities::Model {
        id,
        user_id: owner,
        activity_import_id: import_id,
        ..test_support::activity()
    }
    .into();
    active.insert(db).await.unwrap();
}

#[tokio::test]
async fn import_selection_rejects_foreign_links_and_falls_back_to_owned_reverse_links() {
    let db = crate::pipeline_fixture_tests::database().await;
    store(&db, 42, 8, None).await;
    store(&db, 43, 7, Some(18)).await;
    let import = activity_imports::Model {
        activity_id: Some(42),
        ..test_support::activity_import()
    };
    let selected = load_activity_for_import(&db, 7, &import)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.id, 43);
    assert_eq!(selected.user_id, 7);
    let selected = load_activity_for_import(&db, 8, &import)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.id, 42);
    assert!(load_activity_for_import(&db, 9, &import)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn import_task_lookup_requires_the_owner_and_completed_imports_skip_processing() {
    let db = crate::pipeline_fixture_tests::database().await;
    let active: activity_imports::ActiveModel = test_support::activity_import().into();
    let import = active.insert(&db).await.unwrap();
    let task: ProcessActivityImportTask =
        serde_json::from_value(serde_json::json!({"user_id":7,"import_id":18})).unwrap();
    assert_eq!(
        load_import_for_task(&db, &task).await.unwrap().id,
        import.id
    );
    let foreign: ProcessActivityImportTask =
        serde_json::from_value(serde_json::json!({"user_id":8,"import_id":18})).unwrap();
    assert!(load_import_for_task(&db, &foreign)
        .await
        .unwrap_err()
        .to_string()
        .contains("not found"));
    ProcessActivityImport::with_uploads_dir(db, "/unused".into())
        .process(1, serde_json::json!({"data":{"user_id":7,"import_id":18}}))
        .await
        .unwrap();
}
