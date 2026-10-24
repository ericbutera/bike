use super::*;
use crate::test_support as fixtures;
use sea_orm::{DatabaseBackend, MockDatabase};

#[tokio::test]
async fn reprocessing_preserves_saved_details_and_checks_the_source_version() {
    let activity = activities::Model {
        source: "archive_url_import".into(),
        title: "My saved ride title".into(),
        activity_type: "race".into(),
        sport: "hike".into(),
        derived_data_json: Some(
            crate::activity_data::StoredActivityDerivedData::default().with_source_metadata(
                crate::activity_data::ActivitySourceMetadata {
                    location: Some("Saved location".into()),
                    description: Some("Saved description".into()),
                    ..Default::default()
                },
            ),
        ),
        ..fixtures::reprocess_activity()
    };
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![activity.clone()]])
        .into_connection();
    let mut parsed = fixtures::parsed_reprocess_activity();
    parsed.draft.title = "Embedded source title".into();
    let profile = TrainingProfile::default();
    let artifact = crate::activity_import_pipeline::load_best_activity_parsing_artifact(
        &MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![fixtures::reprocess_artifact()]])
            .into_connection(),
        &fixtures::reprocess_import(),
    )
    .await
    .unwrap();
    activity
        .clone()
        .update_parsed(
            &db,
            &artifact,
            &parsed,
            build_save_data(&parsed, "outdoor.tcx", &profile).unwrap(),
        )
        .await
        .unwrap();
    let transactions = db.into_transaction_log();
    let write = transactions
        .iter()
        .flat_map(|transaction| transaction.statements())
        .find(|statement| statement.sql.starts_with("UPDATE \"activities\""))
        .unwrap();
    let values = &write.values.as_ref().unwrap().0;
    assert!(!write.sql.contains("\"source\" ="));
    assert!(values.contains(&sea_orm::Value::from("My saved ride title")));
    assert!(values.contains(&sea_orm::Value::from("race")));
    assert!(values.contains(&sea_orm::Value::from("hike")));
    assert!(!values.contains(&sea_orm::Value::from("Embedded source title")));
    let log = format!("{transactions:?}");
    assert!(log.contains("Saved location"));
    assert!(log.contains("Saved description"));
    assert!(log.contains("WHERE"));
    assert!(log.contains("updated_at\\\" ="));
    let expected = format!("{:?}", sea_orm::Value::from(activity.updated_at));
    assert!(log.contains(&expected), "expected {expected} in {log}");
}
