use super::{activities, activity_import_artifacts, activity_imports};
use crate::test_support as fixtures;

#[tokio::test]
async fn an_explicit_owned_activity_link_takes_precedence_over_a_reverse_link() {
    let import = activity_imports::Model {
        activity_id: Some(99),
        ..fixtures::activity_import()
    };
    let reverse = activities::Model {
        activity_import_id: Some(import.id),
        ..fixtures::activity()
    };
    let explicit = activities::Model {
        id: 99,
        ..fixtures::activity()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![reverse], vec![explicit.clone()]])
        .into_connection();
    let found =
        activities::Model::linked_to_imports(&db, import.user_id, std::slice::from_ref(&import))
            .await
            .unwrap();
    assert_eq!(found[&import.id], explicit);
    for txn in db.into_transaction_log() {
        let query = &txn.statements()[0];
        assert!(query.sql.contains("\"user_id\" ="));
        assert!(query
            .values
            .as_ref()
            .unwrap()
            .0
            .contains(&import.user_id.into()));
    }
}

#[tokio::test]
async fn a_missing_explicit_link_preserves_the_owned_reverse_link() {
    let import = activity_imports::Model {
        activity_id: Some(99),
        ..fixtures::activity_import()
    };
    let reverse = activities::Model {
        activity_import_id: Some(import.id),
        ..fixtures::activity()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![reverse.clone()], vec![]])
        .into_connection();
    let found =
        activities::Model::linked_to_imports(&db, import.user_id, std::slice::from_ref(&import))
            .await
            .unwrap();
    assert_eq!(found[&import.id], reverse);
}

#[tokio::test]
async fn import_history_applies_ownership_filters_and_stable_pagination() {
    let db = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::count_row(1)]])
        .append_query_results([vec![fixtures::activity_import()]])
        .into_connection();
    let (items, total) = activity_imports::Entity::history(
        &db,
        7,
        2,
        Some("manual_upload"),
        Some("processed"),
        Some(20),
    )
    .await
    .unwrap();
    assert_eq!((items.len(), total), (1, 1));
    let log = db.into_transaction_log();
    let query = &log.last().unwrap().statements()[0];
    assert!(query
        .sql
        .contains("ORDER BY \"activity_imports\".\"id\" DESC"));
    assert_eq!(
        query.values.as_ref().unwrap().0,
        vec![
            7.into(),
            "manual_upload".into(),
            "processed".into(),
            20.into(),
            25_u64.into(),
            25_u64.into()
        ]
    );
}

#[tokio::test]
async fn retained_download_prefers_the_best_original_and_newest_equal_quality() {
    let preferred = activity_import_artifacts::Model {
        id: 24,
        format: "fit".into(),
        source_quality: "fit_original".into(),
        ..fixtures::import_artifact()
    };
    let db = fixtures::postgres_mock()
        .append_query_results([vec![
            fixtures::import_artifact(),
            activity_import_artifacts::Model {
                id: 21,
                artifact_kind: "generated_export".into(),
                ..preferred.clone()
            },
            activity_import_artifacts::Model {
                id: 22,
                format: "json".into(),
                ..preferred.clone()
            },
            activity_import_artifacts::Model {
                id: 23,
                ..preferred.clone()
            },
            preferred.clone(),
        ]])
        .into_connection();
    assert_eq!(
        activity_import_artifacts::Entity::downloadable_original(&db, 7, 18)
            .await
            .unwrap(),
        Some(preferred)
    );
    let log = db.into_transaction_log();
    let query = &log[0].statements()[0];
    assert!(query.sql.contains("\"user_id\" =") && query.sql.contains("\"activity_import_id\" ="));
    assert!(query
        .sql
        .contains("ORDER BY \"activity_import_artifacts\".\"id\" ASC"));
}
