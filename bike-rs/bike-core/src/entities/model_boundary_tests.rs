use super::{
    activities, activity_archive_import_jobs, activity_import_artifacts, activity_imports,
};
use crate::activity_sport::ActivitySport;
use crate::activity_type::ActivityType;
use crate::platform::data::pagination::PaginationParams;
use crate::test_support;
use crate::test_support::table_with_timestamp_defaults;
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DbBackend, DbErr, EntityTrait, MockDatabase,
    Schema,
};
pub(super) async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(activities::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(activity_imports::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(activity_import_artifacts::Entity))
        .await
        .unwrap();
    db.execute(&schema.create_table_from_entity(activity_archive_import_jobs::Entity))
        .await
        .unwrap();
    db
}

async fn store_activity(db: &DatabaseConnection, activity: activities::Model) -> activities::Model {
    let active: activities::ActiveModel = activity.into();
    test_support::insert(db, active).await
}

#[tokio::test]
async fn activity_pages_keep_owner_sport_totals_and_tied_start_order() {
    let db = database().await;
    for (id, user_id, sport) in [
        (1, 7, "ride"),
        (2, 7, "Ride"),
        (3, 8, "ride"),
        (4, 7, "run"),
    ] {
        store_activity(
            &db,
            activities::Model {
                id,
                user_id,
                sport: sport.into(),
                ..test_support::activity()
            },
        )
        .await;
    }
    let mut pagination = PaginationParams {
        page: 1,
        per_page: 1,
    };
    let first =
        activities::Model::paginate_for_user(&db, 7, &pagination, Some(ActivitySport::RoadRide))
            .await
            .unwrap();
    assert_eq!(first.metadata.total, 2);
    assert_eq!(first.metadata.total_pages, 2);
    assert_eq!(first.data[0].id, 2);
    pagination.page = 2;
    let second =
        activities::Model::paginate_for_user(&db, 7, &pagination, Some(ActivitySport::RoadRide))
            .await
            .unwrap();
    assert_eq!(second.metadata.total, 2);
    assert_eq!(second.data[0].id, 1);
    pagination.page = 3;
    assert!(activities::Model::paginate_for_user(
        &db,
        7,
        &pagination,
        Some(ActivitySport::RoadRide)
    )
    .await
    .unwrap()
    .data
    .is_empty());
    let all = activities::Model::paginate_for_user(
        &db,
        7,
        &PaginationParams {
            page: 0,
            per_page: 200,
        },
        None,
    )
    .await
    .unwrap();
    assert_eq!(all.metadata.page, 1);
    assert_eq!(all.metadata.per_page, 100);
    assert_eq!(all.metadata.total, 3);
    assert_eq!(
        all.data
            .iter()
            .map(|activity| activity.id)
            .collect::<Vec<_>>(),
        vec![4, 2, 1]
    );
    assert!(activities::Model::find_owned(&db, 3, 7)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn import_links_prefer_explicit_owned_activity_and_keep_reverse_fallback() {
    let db = database().await;
    for (id, user_id, activity_import_id) in [
        (1, 7, Some(18)),
        (2, 7, None),
        (3, 8, Some(19)),
        (4, 7, Some(20)),
    ] {
        store_activity(
            &db,
            activities::Model {
                id,
                user_id,
                activity_import_id,
                ..test_support::activity()
            },
        )
        .await;
    }
    let imports = [
        activity_imports::Model {
            activity_id: Some(2),
            ..test_support::activity_import()
        },
        activity_imports::Model {
            id: 19,
            activity_id: Some(3),
            ..test_support::activity_import()
        },
        activity_imports::Model {
            id: 20,
            activity_id: Some(999),
            ..test_support::activity_import()
        },
    ];
    let linked = activities::Model::linked_to_imports(&db, 7, &imports)
        .await
        .unwrap();
    assert_eq!(linked.len(), 2);
    assert_eq!(linked[&18].id, 2);
    assert_eq!(linked[&20].id, 4);
    assert!(!linked.contains_key(&19));
    let reverse = activities::Model::linked_to_imports(&db, 7, &[test_support::activity_import()])
        .await
        .unwrap();
    assert_eq!(reverse[&18].id, 1);
}

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
    MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![test_support::activity()], vec![after]])
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
            activity_type: kind.map_or_else(
                || original.activity_type.clone(),
                |kind| kind.as_str().into(),
            ),
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
        original
            .clone()
            .update_details(&db, None, None)
            .await
            .unwrap(),
        original
    );
    let error = original
        .update_details(&db, Some("  ".into()), Some(ActivityType::Race))
        .await
        .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(
        error.errors.unwrap()["title"],
        vec!["Activity title is required"]
    );
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
async fn recent_manual_uploads_keep_source_owner_order_and_twenty_five_item_limit() {
    let db = database().await;
    for id in 1..=28 {
        let import = activity_imports::Model {
            id,
            user_id: if id == 28 { 8 } else { 7 },
            source: if id == 27 {
                "strava_sync".into()
            } else {
                "manual_upload".into()
            },
            ..test_support::activity_import()
        };
        let mut active: activity_imports::ActiveModel = import.into();
        active.created_at =
            sea_orm::Set(test_support::timestamp() + chrono::Duration::seconds(id.into()));
        test_support::insert(&db, active).await;
    }
    let imports = activity_imports::Entity::recent_manual_uploads(&db, 7)
        .await
        .unwrap();
    assert_eq!(imports.len(), 25);
    assert_eq!(imports.first().unwrap().id, 26);
    assert_eq!(imports.last().unwrap().id, 2);
    assert!(activity_imports::Entity::find_owned(&db, 7, 28)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn archive_jobs_keep_owner_order_and_ten_item_limit() {
    let db = database().await;
    for id in 1..=12 {
        let job = activity_archive_import_jobs::Model {
            id,
            user_id: if id == 12 { 8 } else { 7 },
            ..test_support::archive_import_job()
        };
        let mut active: activity_archive_import_jobs::ActiveModel = job.into();
        active.created_at =
            sea_orm::Set(test_support::timestamp() + chrono::Duration::seconds(id.into()));
        test_support::insert(&db, active).await;
    }
    let jobs = activity_archive_import_jobs::Entity::recent_for_user(&db, 7)
        .await
        .unwrap();
    assert_eq!(jobs.len(), 10);
    assert_eq!(jobs.first().unwrap().id, 11);
    assert_eq!(jobs.last().unwrap().id, 2);
    assert_eq!(
        activity_archive_import_jobs::Entity::find_owned(&db, 7, 11)
            .await
            .unwrap()
            .unwrap()
            .id,
        11
    );
    assert!(activity_archive_import_jobs::Entity::find_owned(&db, 7, 12)
        .await
        .unwrap()
        .is_none());
    assert!(
        activity_archive_import_jobs::Entity::find_owned(&db, 7, 999)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn downloadable_originals_keep_quality_order_owner_kind_and_supported_formats() {
    let db = database().await;
    for (id, format, quality, kind, user_id, import_id) in [
        (1, "gpx", "unknown", "original", 7, 18),
        (2, "gpx", "gpx_original", "original", 7, 18),
        (3, "tcx", "unknown", "original", 7, 18),
        (4, "tcx", "tcx_original", "original", 7, 18),
        (5, "fit", "unknown", "original", 7, 18),
        (6, "fit", "fit_original", "original", 7, 18),
        (7, "fit", "fit_original", "original", 7, 18),
        (8, "fit", "fit_original", "original", 8, 18),
        (9, "fit", "fit_original", "original", 7, 19),
        (10, "json", "fit_original", "original", 7, 18),
        (11, "fit", "fit_original", "generated_export", 7, 18),
    ] {
        let artifact = activity_import_artifacts::Model {
            id,
            format: format.into(),
            source_quality: quality.into(),
            artifact_kind: kind.into(),
            user_id,
            activity_import_id: import_id,
            ..test_support::import_artifact()
        };
        let active: activity_import_artifacts::ActiveModel = artifact.into();
        test_support::insert(&db, active).await;
    }
    for expected in [7, 6, 5, 4, 3, 2, 1] {
        let artifact = activity_import_artifacts::Entity::downloadable_original(&db, 7, 18)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(artifact.id, expected);
        activity_import_artifacts::Entity::delete_by_id(expected)
            .exec(&db)
            .await
            .unwrap();
    }
    assert!(
        activity_import_artifacts::Entity::downloadable_original(&db, 7, 18)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        activity_import_artifacts::Entity::downloadable_original(&db, 7, 999)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn report_reads_keep_owner_cycling_and_duration_or_total_time_policy() {
    let db = database().await;
    for (id, user_id, sport, distance) in [
        (1, 7, "ride", 5000.0),
        (2, 7, "run", 5000.0),
        (3, 8, "ride", 5000.0),
        (4, 7, "Ride", 1000.0),
    ] {
        store_activity(
            &db,
            activities::Model {
                id,
                user_id,
                sport: sport.into(),
                moving_time_seconds: Some(120),
                total_time_seconds: Some(240),
                distance_meters: Some(distance),
                ..test_support::activity()
            },
        )
        .await;
    }
    let at = test_support::timestamp();
    let filtered = activities::Model::report_range(&db, 7, at, at, Some(180), Some(2000.0))
        .await
        .unwrap();
    assert_eq!(
        filtered.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );
    let selected = activities::Model::report_selection(&db, 7, &[1, 2, 3], None, None)
        .await
        .unwrap();
    assert_eq!(
        selected.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );
    let all = activities::Model::report_range(&db, 7, at, at, Some(-1), Some(-1.0))
        .await
        .unwrap();
    assert_eq!(all.len(), 2);
}

fn segment_effort_table(db: &DatabaseConnection) -> sea_orm::sea_query::TableCreateStatement {
    table_with_timestamp_defaults(
        db,
        super::segment_efforts::Entity,
        &["created_at", "updated_at"],
    )
}

pub(super) async fn segment_database() -> DatabaseConnection {
    use super::{activity_analytics, segment_summaries, segment_user_summaries, segments};
    let db = database().await;
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        table_with_timestamp_defaults(&db, segments::Entity, &["last_activity_change_at"]),
        segment_effort_table(&db),
        schema.create_table_from_entity(segment_summaries::Entity),
        schema.create_table_from_entity(segment_user_summaries::Entity),
        schema.create_table_from_entity(activity_analytics::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    db
}

#[tokio::test]
async fn segment_deletion_rolls_back_all_related_rows_and_preserves_the_activity() {
    use super::{segment_efforts, segment_summaries, segment_user_summaries, segments};
    use sea_orm::{PaginatorTrait, Set};
    let db = segment_database().await;
    let active: segments::ActiveModel = test_support::segment().into();
    let segment = test_support::insert(&db, active).await;
    store_activity(&db, test_support::activity()).await;
    test_support::insert(&db, test_support::new_segment_effort()).await;
    test_support::insert(
        &db,
        segment_summaries::ActiveModel {
            segment_id: Set(segment.id),
            effort_count: Set(1),
            ..Default::default()
        },
    )
    .await;
    test_support::insert(
        &db,
        segment_user_summaries::ActiveModel {
            segment_id: Set(segment.id),
            user_id: Set(segment.user_id),
            effort_count: Set(1),
            ..Default::default()
        },
    )
    .await;
    db.execute_unprepared("CREATE TRIGGER reject_segment_delete BEFORE DELETE ON segments BEGIN SELECT RAISE(ABORT, 'fixture delete failure'); END").await.unwrap();
    assert!(segment.delete_with_derived_state(&db).await.is_err());
    assert_eq!(
        segment_summaries::Entity::find().count(&db).await.unwrap(),
        1
    );
    assert_eq!(
        segment_user_summaries::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        1
    );
    assert_eq!(segment_efforts::Entity::find().count(&db).await.unwrap(), 1);
    assert!(segments::Model::find_owned(&db, segment.id, 8)
        .await
        .unwrap()
        .is_none());
    db.execute_unprepared("DROP TRIGGER reject_segment_delete")
        .await
        .unwrap();
    segment.delete_with_derived_state(&db).await.unwrap();
    assert_eq!(
        segment_summaries::Entity::find().count(&db).await.unwrap(),
        0
    );
    assert_eq!(
        segment_user_summaries::Entity::find()
            .count(&db)
            .await
            .unwrap(),
        0
    );
    assert_eq!(segment_efforts::Entity::find().count(&db).await.unwrap(), 0);
    assert!(segments::Model::find_owned(&db, segment.id, 7)
        .await
        .unwrap()
        .is_none());
    assert!(activities::Model::find_owned(&db, 42, 7)
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn segment_and_fitness_insert_fixtures_receive_generated_ids() {
    use super::fitness_freshness_daily;
    let db = segment_database().await;
    db.execute(
        &Schema::new(db.get_database_backend())
            .create_table_from_entity(fitness_freshness_daily::Entity),
    )
    .await
    .unwrap();
    let a = test_support::insert(&db, test_support::new_segment_effort()).await;
    let b = test_support::insert(&db, test_support::new_segment_effort()).await;
    assert_ne!(a.id, b.id);
    let a = test_support::insert(&db, test_support::new_fitness_day()).await;
    let b = test_support::insert(&db, test_support::new_fitness_day()).await;
    assert_ne!(a.id, b.id);
    assert_eq!(a.user_id, b.user_id);
}

#[tokio::test]
async fn segment_dedupe_reads_keep_owner_and_route_bucket_policy() {
    use super::segments;
    let db = segment_database().await;
    let points = crate::activity_details::derive_activity_detail_data(
        "fixture.gpx",
        "gpx",
        include_bytes!("../../../api/tests/fixtures/import-analytics-ride.gpx"),
    )
    .unwrap()
    .route_points;
    assert!(points.len() > 1);
    for (id, user_id) in [(1, 7), (2, 8)] {
        let active: segments::ActiveModel = segments::Model {
            id,
            user_id,
            route_data_json: Some(crate::segment_support::serialize_segment_route_points(
                &points,
            )),
            ..test_support::segment()
        }
        .into();
        test_support::insert(&db, active).await;
    }
    assert_eq!(
        segments::Model::find_duplicate(&db, 7, Some(1000.0), &points)
            .await
            .unwrap()
            .unwrap()
            .id,
        1
    );
    assert_eq!(
        segments::Model::find_duplicate(&db, 8, Some(1000.0), &points)
            .await
            .unwrap()
            .unwrap()
            .id,
        2
    );
    assert!(
        segments::Model::find_duplicate(&db, 9, Some(1000.0), &points)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        segments::Model::find_duplicate(&db, 7, Some(1010.0), &points)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn training_projections_preserve_owner_sport_selection_and_summary_fields() {
    let db = database().await;
    for (id, user_id, sport, kind) in [
        (1, 7, "ride", "race"),
        (2, 7, "run", "race"),
        (3, 8, "ride", "race"),
        (4, 7, "road_ride", "training"),
    ] {
        store_activity(
            &db,
            activities::Model {
                id,
                user_id,
                sport: sport.into(),
                activity_type: kind.into(),
                distance_meters: Some(1234.0),
                moving_time_seconds: Some(120),
                ..test_support::activity()
            },
        )
        .await;
    }
    let races = activities::Model::race_summaries(&db, 7).await.unwrap();
    assert_eq!(races.iter().map(|row| row.id).collect::<Vec<_>>(), vec![1]);
    assert_eq!(races[0].distance_meters, Some(1234.0));
    let training = activities::Model::training_summaries(&db, 7, &[1, 2, 3, 4])
        .await
        .unwrap();
    assert_eq!(training.len(), 2);
    assert_eq!(training[&4].moving_time_seconds, Some(120));
    assert!(activities::Model::training_summaries(&db, 7, &[])
        .await
        .unwrap()
        .is_empty());
    let summaries = activities::Model::effort_summaries(&db, &[1, 4])
        .await
        .unwrap();
    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].title, "Fixture ride");
    let sources = activities::Model::effort_sources(&db, &[4]).await.unwrap();
    assert_eq!(sources[0].id, 4);
    assert_eq!(sources[0].derived_data_json, None);
}

async fn seed_segment_reads(db: &DatabaseConnection) {
    use super::{segment_efforts, segments};
    for (id, user_id, mode) in [(1, 7, "dh"), (2, 8, "dh"), (3, 7, "xc")] {
        let active: segments::ActiveModel = segments::Model {
            id,
            user_id,
            mode: mode.into(),
            ..test_support::segment()
        }
        .into();
        test_support::insert(db, active).await;
    }
    for (id, user_id, segment_id, duration_seconds) in [(1, 7, 1, 70), (2, 7, 1, 60), (3, 8, 2, 30)]
    {
        let active: segment_efforts::ActiveModel = segment_efforts::Model {
            id,
            user_id,
            segment_id,
            duration_seconds,
            ..test_support::segment_effort()
        }
        .into();
        test_support::insert(db, active).await;
    }
}

#[tokio::test]
async fn segment_reads_keep_downhill_ownership_and_effort_ranking() {
    use super::{segment_efforts, segments};
    let db = segment_database().await;
    seed_segment_reads(&db).await;
    assert_eq!(
        segments::Model::downhill_for_user(&db, 7).await.unwrap()[0].id,
        1
    );
    let selected = segments::Model::downhill_by_ids(&db, &[1, 3])
        .await
        .unwrap();
    assert_eq!(
        selected.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![1]
    );
    assert_eq!(segments::Model::all_ids(&db).await.unwrap(), vec![1, 2, 3]);
    let ranked = segment_efforts::Model::ranked_for_user_segment(&db, 7, 1)
        .await
        .unwrap();
    assert_eq!(
        ranked
            .iter()
            .map(|row| row.duration_seconds)
            .collect::<Vec<_>>(),
        vec![60, 70]
    );
    let all = segment_efforts::Model::ranked_for_segments(&db, &[1, 2])
        .await
        .unwrap();
    assert_eq!(
        all.iter().map(|row| row.id).collect::<Vec<_>>(),
        vec![3, 2, 1]
    );
    let owned = segment_efforts::Model::for_owned_segments(&db, 7, &[1, 2])
        .await
        .unwrap();
    assert_eq!(owned.len(), 2);
    assert!(owned.iter().all(|row| row.user_id == 7));
    assert_eq!(
        segment_efforts::Model::activity_ids_for_segments(&db, &[1, 2])
            .await
            .unwrap(),
        vec![42]
    );
    assert_eq!(
        segment_efforts::Model::for_activity_ids(&db, &[42])
            .await
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn matched_effort_storage_preserves_indices_and_assigns_effort_order() {
    use super::segment_efforts;
    use crate::segment_support::MatchedSegmentEffort;
    let db = segment_database().await;
    let matches = [MatchedSegmentEffort {
        start_route_point_index: 10,
        end_route_point_index: 20,
        start_elapsed_seconds: 30,
        end_elapsed_seconds: 90,
        duration_seconds: 60,
        distance_meters: Some(500.0),
    }];
    segment_efforts::Entity::store_matches(&db, 7, 1, 42, &[])
        .await
        .unwrap();
    assert!(segment_efforts::Model::for_activity_ids(&db, &[42])
        .await
        .unwrap()
        .is_empty());
    segment_efforts::Entity::store_matches(&db, 7, 1, 42, &matches)
        .await
        .unwrap();
    let stored = segment_efforts::Model::ranked_for_user_segment(&db, 7, 1)
        .await
        .unwrap();
    assert_eq!(stored[0].effort_index, 1);
    assert_eq!(
        (
            stored[0].start_route_point_index,
            stored[0].end_route_point_index
        ),
        (10, 20)
    );
    assert_eq!(
        (stored[0].duration_seconds, stored[0].distance_meters),
        (60, Some(500.0))
    );
}

#[tokio::test]
async fn oauth_model_operations_preserve_identity_linking() {
    use crate::auth::entities::users;
    let db = database().await;
    db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(users::Entity))
        .await
        .unwrap();
    let active: users::ActiveModel = test_support::customer().into();
    let user = test_support::insert(&db, active).await;
    let linked = user
        .link_oauth_identity(&db, "fixture", "subject-7".into())
        .await
        .unwrap();
    assert_eq!(linked.email, user.email);
    assert_eq!(
        users::Model::by_provider_identity(&db, "fixture", "subject-7")
            .await
            .unwrap()
            .unwrap()
            .id,
        7
    );
    assert!(
        users::Model::by_provider_identity(&db, "other", "subject-7")
            .await
            .unwrap()
            .is_none()
    );
    assert!(users::Model::find_optional(&db, 999)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        users::Model::rider_names(&db, &[7]).await.unwrap()[0].name,
        user.name
    );
    linked.ensure_admin(&db).await.unwrap();
    let admin = users::Model::find_optional(&db, linked.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(admin.is_admin, Some(true));
    admin.ensure_admin(&db).await.unwrap();
    assert_eq!(
        users::Model::find_optional(&db, linked.id).await.unwrap(),
        Some(admin)
    );
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
    let created = users::Model::create_oauth_user(&db, "fixture", provider)
        .await
        .unwrap();
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

#[tokio::test]
async fn feature_flag_model_and_service_keep_storage_and_cache_consistent() {
    use crate::platform::feature_flags::{entities as flags, service::FeatureFlagService};
    use sea_orm::Set;
    let db = database().await;
    db.execute(&Schema::new(db.get_database_backend()).create_table_from_entity(flags::Entity))
        .await
        .unwrap();
    test_support::insert(
        &db,
        flags::ActiveModel {
            feature_key: Set("heatmaps".into()),
            ..test_support::new_feature_flag()
        },
    )
    .await;
    let service = FeatureFlagService::new();
    service.load_cache(&db).await.unwrap();
    assert!(!service.is_enabled("heatmaps"));
    assert!(!flags::Model::enabled(&db, "missing").await.unwrap());
    let updated = service.update_flag(&db, "heatmaps", true).await.unwrap();
    assert!(updated.enabled);
    assert!(service.is_enabled("heatmaps"));
    assert!(crate::heatmaps::projection::Projection::enabled(&db)
        .await
        .unwrap());
    assert_eq!(FeatureFlagService::list_all(&db).await.unwrap().len(), 1);
    assert!(service.update_flag(&db, "missing", true).await.is_err());
}
