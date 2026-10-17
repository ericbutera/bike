use super::activities;
use crate::activity_type::ActivityType;
use crate::test_support;
use sea_orm::{DbBackend, DbErr, MockDatabase};

#[tokio::test]
async fn empty_import_links_do_not_query_and_database_errors_propagate() {
    let db = MockDatabase::new(DbBackend::Postgres).into_connection();
    assert!(activities::Model::linked_to_imports(&db, 7, &[])
        .await
        .unwrap()
        .is_empty());
    assert!(db.into_transaction_log().is_empty());
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_errors([DbErr::Custom("query unavailable".into())])
        .into_connection();
    let error = activities::Model::linked_to_imports(&db, 7, &[test_support::activity_import()])
        .await
        .unwrap_err();
    assert_eq!(error, DbErr::Custom("query unavailable".into()));
}

fn activity_edit_fixture(after: activities::Model) -> MockDatabase {
    MockDatabase::new(DbBackend::Postgres).append_query_results([vec![test_support::activity()], vec![after]])
}

#[tokio::test]
async fn updating_activity_details_preserves_untouched_fields_and_archives_the_change() {
    let original = test_support::activity();
    for (title, kind) in [
        (Some("  Lunch Loop  "), Some(ActivityType::Race)),
        (Some("  Lunch Loop  "), None),
        (None, Some(ActivityType::Race)),
    ] {
        let updated = activities::Model {
            title: title.map_or_else(|| original.title.clone(), |title| title.trim().into()),
            activity_type: kind.map_or_else(|| original.activity_type.clone(), |kind| kind.as_str().into()),
            ..original.clone()
        };
        let db = activity_edit_fixture(updated.clone())
            .append_exec_results([sea_orm::MockExecResult {
                last_insert_id: 0,
                rows_affected: 1,
            }])
            .into_connection();
        assert_eq!(
            original
                .clone()
                .update_details(&db, title.map(str::to_owned), kind)
                .await
                .unwrap(),
            updated,
        );
        let log = db.into_transaction_log();
        assert_eq!(log.len(), 1);
        assert!(log[0]
            .statements()
            .iter()
            .any(|statement| statement.sql.contains("activity_event_history")));
    }
}

#[tokio::test]
async fn unchanged_or_invalid_manual_edits_never_archive_a_change() {
    let original = test_support::activity();
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![original.clone()]])
        .into_connection();
    assert_eq!(
        original.clone().update_details(&db, None, None).await.unwrap(),
        original
    );
    let error = original
        .update_details(&db, Some("  ".into()), Some(ActivityType::Race))
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(error.errors.unwrap()["title"], vec!["Activity title is required"]);
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn failed_manual_edit_history_rolls_back_the_activity_change() {
    let before = test_support::activity();
    let after = activities::Model {
        title: "New title".into(),
        ..before.clone()
    };
    let db = activity_edit_fixture(after)
        .append_exec_errors([DbErr::Custom("fixture archive unavailable".into())])
        .into_connection();
    assert!(before
        .update_details(&db, Some("New title".into()), None)
        .await
        .is_err());
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn oauth_user_creation_maps_provider_identity_and_display_name() {
    use crate::auth::{entities::users, services::oauth::OAuthUserInfo};
    let created = users::Model {
        id: 8,
        email: "new@example.test".into(),
        name: "New rider".into(),
        oauth_subject: Some("subject-new".into()),
        oauth_provider: Some("fixture".into()),
        api_key: "new-key".into(),
        ..test_support::customer()
    };
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![created]])
        .into_connection();
    let provider: OAuthUserInfo = serde_json::from_value(serde_json::json!({
        "sub":"subject-new", "email":"new@example.test", "preferred_username":"New rider"
    }))
    .unwrap();
    let created = users::Model::create_oauth_user(&db, "fixture", provider).await.unwrap();
    assert_eq!(created.name, "New rider");
    assert_eq!(created.oauth_subject.as_deref(), Some("subject-new"));
    assert!(created.password.is_none());
    assert!(created.email_verified_at.is_some());
    let log = format!("{:?}", db.into_transaction_log());
    for evidence in [
        "INSERT INTO",
        "RETURNING",
        "New rider",
        "subject-new",
        "new@example.test",
    ] {
        assert!(log.contains(evidence), "missing {evidence}: {log}");
    }
}
