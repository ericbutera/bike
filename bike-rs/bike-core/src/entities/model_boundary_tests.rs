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
                .update_details(
                    &db,
                    activities::ActivityDetailsUpdate {
                        title: title.map(str::to_owned),
                        activity_type: kind,
                        ..Default::default()
                    }
                )
                .await
                .unwrap()
                .activity,
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
        original
            .clone()
            .update_details(&db, Default::default())
            .await
            .unwrap()
            .activity,
        original
    );
    let error = original
        .update_details(
            &db,
            activities::ActivityDetailsUpdate {
                title: Some("  ".into()),
                activity_type: Some(ActivityType::Race),
                ..Default::default()
            },
        )
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
        .update_details(
            &db,
            activities::ActivityDetailsUpdate {
                title: Some("New title".into()),
                ..Default::default()
            }
        )
        .await
        .is_err());
    let log = db.into_transaction_log();
    assert_eq!(log[0].statements().last().unwrap().sql, "ROLLBACK");
}

#[tokio::test]
async fn manual_metadata_edits_preserve_recordings_and_gear_and_allow_clearing() {
    use crate::activity_data::{ActivityGear, ActivitySourceMetadata};
    let stored =
        crate::activity_data::serialize_derived_activity_data(&test_support::parsed_reprocess_activity().derived_data);
    let gear = Some(ActivityGear {
        id: "fixture-bike".into(),
        name: Some("Trail bike".into()),
    });
    for text in ["  After ride  ", ""] {
        let before = activities::Model {
            derived_data_json: Some(stored.clone().with_source_metadata(ActivitySourceMetadata {
                description: Some("Before ride".into()),
                private_notes: Some("Personal note".into()),
                gear: gear.clone(),
                location: Some("Old city".into()),
            })),
            ..test_support::activity()
        };
        let value = (!text.trim().is_empty()).then(|| text.trim().to_string());
        let after = activities::Model {
            derived_data_json: Some(stored.clone().with_source_metadata(ActivitySourceMetadata {
                description: value.clone(),
                private_notes: value.clone(),
                gear: gear.clone(),
                location: value.clone(),
            })),
            ..before.clone()
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![before.clone()], vec![after.clone()]])
            .append_exec_results([sea_orm::MockExecResult {
                rows_affected: 1,
                ..Default::default()
            }])
            .into_connection();
        let result = before
            .update_details(
                &db,
                activities::ActivityDetailsUpdate {
                    description: Some(text.into()),
                    private_notes: Some(text.into()),
                    location: Some(text.into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result.activity, after);
        assert!(!result.sport_changed);
        let log = db.into_transaction_log();
        let write = log[0]
            .statements()
            .iter()
            .find(|statement| statement.sql.starts_with("UPDATE \"activities\""))
            .unwrap();
        assert!(!write.sql.contains("\"sport\" ="));
        let stored_json = serde_json::to_value(after.derived_data_json.unwrap()).unwrap();
        assert!(write
            .values
            .as_ref()
            .unwrap()
            .0
            .contains(&sea_orm::Value::Json(Some(Box::new(stored_json)))));
        assert!(log[0]
            .statements()
            .iter()
            .any(|statement| statement.sql.contains("activity_event_history")));
    }
}

#[tokio::test]
async fn sport_edits_update_eligibility_and_archive_within_the_transaction() {
    use crate::activity_sport::ActivitySport;
    for (old_sport, sport, affects_cycling) in [
        ("road_ride", ActivitySport::Hike, true),
        ("road_ride", ActivitySport::MountainBike, true),
        ("walk", ActivitySport::Hike, false),
        ("hike", ActivitySport::RoadRide, true),
    ] {
        let before = activities::Model {
            sport: old_sport.into(),
            estimated_ftp_watts: Some(250),
            ..test_support::activity()
        };
        let after = activities::Model {
            sport: sport.as_str().into(),
            estimated_ftp_watts: if sport.is_cycling() {
                before.estimated_ftp_watts
            } else {
                None
            },
            ..before.clone()
        };
        let deletes = if sport.is_cycling() { 0 } else { 3 };
        let writes = deletes + usize::from(affects_cycling) + 1;
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![before.clone()], vec![after.clone()]])
            .append_exec_results((0..writes).map(|_| sea_orm::MockExecResult {
                rows_affected: 1,
                ..Default::default()
            }))
            .into_connection();
        let result = before
            .update_details(
                &db,
                activities::ActivityDetailsUpdate {
                    sport: Some(sport),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(result.activity, after);
        assert!(result.sport_changed);
        assert_eq!(result.affects_cycling, affects_cycling);
        let log = db.into_transaction_log();
        let statements = log[0].statements();
        assert_eq!(
            statements
                .iter()
                .filter(|statement| statement.sql.starts_with("DELETE"))
                .count(),
            deletes
        );
        assert_eq!(
            statements
                .iter()
                .any(|statement| statement.sql.contains("analytics_user_states")),
            affects_cycling
        );
        assert!(statements
            .iter()
            .any(|statement| statement.sql.contains("activity_event_history")));
        assert_eq!(statements.last().unwrap().sql, "COMMIT");
    }
}

#[tokio::test]
async fn gear_name_edits_preserve_the_identifier_and_other_metadata() {
    use crate::activity_data::{ActivityGear, ActivitySourceMetadata, StoredActivityDerivedData};
    for name in ["  Winter bike  ", ""] {
        let metadata = ActivitySourceMetadata {
            gear: Some(ActivityGear {
                id: "bike-7".into(),
                name: Some("Trail bike".into()),
            }),
            description: Some("Trail loop".into()),
            private_notes: Some("Personal note".into()),
            ..Default::default()
        };
        let before = activities::Model {
            derived_data_json: Some(StoredActivityDerivedData::default().with_source_metadata(metadata.clone())),
            ..test_support::activity()
        };
        let mut edited = metadata;
        edited.gear.as_mut().unwrap().name = (!name.trim().is_empty()).then(|| name.trim().to_owned());
        let stored = StoredActivityDerivedData::default().with_source_metadata(edited);
        let after = activities::Model {
            derived_data_json: Some(stored.clone()),
            ..before.clone()
        };
        let db = MockDatabase::new(DbBackend::Postgres)
            .append_query_results([vec![before.clone()], vec![after]])
            .append_exec_results([sea_orm::MockExecResult {
                rows_affected: 1,
                ..Default::default()
            }])
            .into_connection();
        before
            .update_details(
                &db,
                activities::ActivityDetailsUpdate {
                    gear_name: Some(name.into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let log = db.into_transaction_log();
        let write = log[0]
            .statements()
            .iter()
            .find(|statement| statement.sql.starts_with("UPDATE \"activities\""))
            .unwrap();
        let expected = sea_orm::Value::Json(Some(Box::new(serde_json::to_value(stored).unwrap())));
        assert!(write.values.as_ref().unwrap().0.contains(&expected));
    }
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
