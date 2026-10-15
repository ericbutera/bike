use crate::storage::AppStorage;
use axum::body::{to_bytes, Body};
use axum::http::Request;
use axum::response::Response;
use axum::Router;
use bike_core::auth::entities::users;
use bike_core::entities::{
    activities, activity_analytics, activity_archive_import_jobs, activity_import_artifacts,
    activity_imports, activity_training_analyses, segment_efforts, segment_summaries,
    segment_user_summaries, segments, user_preferences,
};
use bike_core::test_support;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, Schema};
use serde_json::{json, Value};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

struct Fixture {
    db: DatabaseConnection,
    app: Router,
    _uploads: TempDir,
}

impl Fixture {
    async fn new() -> Self {
        let db = database().await;
        seed(&db).await;
        let uploads = tempfile::tempdir().unwrap();
        std::fs::create_dir(uploads.path().join("activity-imports")).unwrap();
        std::fs::write(
            uploads.path().join("activity-imports/fixture.gpx"),
            "<gpx/>",
        )
        .unwrap();
        let mut state = AppStorage::for_test(db.clone());
        state.local_admin_user_pid = Some(test_support::customer().pid);
        state.uploads_dir = uploads.path().to_str().unwrap().into();
        Self {
            db,
            app: super::routes().with_state(Arc::new(state)),
            _uploads: uploads,
        }
    }

    async fn request(&self, method: &str, path: &str, body: Value) -> Response {
        self.app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn json(&self, method: &str, path: &str, body: Value, status: u16) -> Value {
        let response = self.request(method, path, body).await;
        let actual_status = response.status();
        let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
        assert_eq!(
            actual_status,
            status,
            "{method} {path}: {}",
            String::from_utf8_lossy(&bytes)
        );
        serde_json::from_slice(&bytes).unwrap()
    }
}

async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        schema.create_table_from_entity(users::Entity),
        schema.create_table_from_entity(activities::Entity),
        schema.create_table_from_entity(activity_imports::Entity),
        schema.create_table_from_entity(activity_import_artifacts::Entity),
        schema.create_table_from_entity(activity_archive_import_jobs::Entity),
        schema.create_table_from_entity(segment_efforts::Entity),
        schema.create_table_from_entity(activity_training_analyses::Entity),
        schema.create_table_from_entity(activity_analytics::Entity),
        schema.create_table_from_entity(segments::Entity),
        schema.create_table_from_entity(segment_summaries::Entity),
        schema.create_table_from_entity(segment_user_summaries::Entity),
        schema.create_table_from_entity(user_preferences::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    db
}

async fn seed(db: &DatabaseConnection) {
    let user: users::ActiveModel = test_support::customer().into();
    user.insert(db).await.unwrap();
    for (id, user_id, import_id, sport) in [
        (42, 7, Some(18), "ride"),
        (43, 8, None, "ride"),
        (44, 7, Some(19), "run"),
    ] {
        let active: activities::ActiveModel = activities::Model {
            id,
            user_id,
            activity_import_id: import_id,
            sport: sport.into(),
            ..test_support::activity()
        }
        .into();
        active.insert(db).await.unwrap();
    }
    for (id, user_id) in [(18, 7), (19, 8)] {
        let active: activity_imports::ActiveModel = activity_imports::Model {
            id,
            user_id,
            ..test_support::activity_import()
        }
        .into();
        active.insert(db).await.unwrap();
    }
    for (id, user_id) in [(20, 7), (21, 8)] {
        let active: activity_archive_import_jobs::ActiveModel =
            activity_archive_import_jobs::Model {
                id,
                user_id,
                ..test_support::archive_import_job()
            }
            .into();
        active.insert(db).await.unwrap();
    }
    let artifact: activity_import_artifacts::ActiveModel = test_support::import_artifact().into();
    artifact.insert(db).await.unwrap();
}

#[tokio::test]
async fn activity_handlers_hide_foreign_and_missing_records() {
    let fixture = Fixture::new().await;
    for id in [43, 999] {
        for (method, suffix) in [
            ("GET", ""),
            ("GET", "/source-file"),
            ("PATCH", ""),
            ("DELETE", ""),
            ("POST", "/regenerate"),
        ] {
            let body = fixture
                .json(
                    method,
                    &format!("/api/activities/{id}{suffix}"),
                    json!({}),
                    404,
                )
                .await;
            assert_eq!(body["message"], "Activity not found");
        }
    }
}

#[tokio::test]
async fn owned_activity_list_detail_update_and_download_keep_the_contract() {
    let fixture = Fixture::new().await;
    let page = fixture
        .json(
            "GET",
            "/api/activities?sport=road_ride&per_page=1",
            json!({}),
            200,
        )
        .await;
    assert_eq!(page["metadata"]["total"], 1);
    assert_eq!(page["data"][0]["id"], 42);
    let detail = fixture
        .json("GET", "/api/activities/42", json!({}), 200)
        .await;
    assert_eq!(detail["can_download_source_file"], true);
    let updated = fixture
        .json(
            "PATCH",
            "/api/activities/42",
            json!({"title": "  Lunch Loop  ", "activity_type": "race"}),
            200,
        )
        .await;
    assert_eq!(updated["title"], "Lunch Loop");
    assert_eq!(updated["activity_type"], "race");
    assert_eq!(updated["source"], detail["source"]);
    let invalid = fixture
        .json(
            "PATCH",
            "/api/activities/42",
            json!({"title": "  ", "activity_type": "training"}),
            400,
        )
        .await;
    assert_eq!(
        invalid["errors"]["title"],
        json!(["Activity title is required"])
    );
    let saved = fixture
        .json("GET", "/api/activities/42", json!({}), 200)
        .await;
    assert_eq!(saved["title"], "Lunch Loop");
    assert_eq!(saved["activity_type"], "race");
    let download = fixture
        .request("GET", "/api/activities/42/source-file", json!({}))
        .await;
    assert_eq!(download.status(), 200);
    assert_eq!(download.headers()["content-type"], "application/gpx+xml");
    assert_eq!(
        download.headers()["content-disposition"],
        "attachment; filename=\"fixture.gpx\""
    );
    assert_eq!(
        to_bytes(download.into_body(), 100).await.unwrap().as_ref(),
        b"<gpx/>"
    );
}

#[tokio::test]
async fn recent_import_and_archive_handlers_return_only_owned_hydrated_records() {
    let fixture = Fixture::new().await;
    let imports = fixture
        .json("GET", "/api/activity-imports", json!({}), 200)
        .await;
    assert_eq!(imports.as_array().unwrap().len(), 1);
    assert_eq!(imports[0]["id"], 18);
    assert_eq!(imports[0]["activity_id"], 42);
    let archives = fixture
        .json("GET", "/api/activity-imports/archive-jobs", json!({}), 200)
        .await;
    assert_eq!(archives.as_array().unwrap().len(), 1);
    assert_eq!(archives[0]["id"], 20);
    let archive = fixture
        .json(
            "GET",
            "/api/activity-imports/archive-jobs/20",
            json!({}),
            200,
        )
        .await;
    assert_eq!(archive["id"], 20);
    fixture
        .json(
            "GET",
            "/api/activity-imports/archive-jobs/21",
            json!({}),
            404,
        )
        .await;
}

#[tokio::test]
async fn regeneration_rejects_an_owned_activity_linked_to_another_users_import() {
    let fixture = Fixture::new().await;
    let error = fixture
        .json("POST", "/api/activities/44/regenerate", json!({}), 404)
        .await;
    assert_eq!(error["message"], "Activity import not found");
}

async fn seed_segment(fixture: &Fixture) -> i32 {
    use sea_orm::Set;
    let derived = bike_core::activity_details::derive_activity_detail_data(
        "fixture.gpx",
        "gpx",
        include_bytes!("../../tests/fixtures/import-analytics-ride.gpx"),
    )
    .unwrap();
    let end = derived.route_points.len() as i32 - 1;
    let distance = derived
        .route_points
        .last()
        .and_then(|point| point.distance_meters);
    let route = bike_core::segment_support::serialize_segment_route_points(&derived.route_points);
    let stored = bike_core::activity_data::serialize_derived_activity_data(&derived);
    activities::ActiveModel {
        id: Set(42),
        derived_data_json: Set(Some(stored)),
        ..Default::default()
    }
    .update(&fixture.db)
    .await
    .unwrap();
    for (id, user_id) in [(1, 7), (2, 8)] {
        let active: segments::ActiveModel = segments::Model {
            id,
            user_id,
            route_data_json: Some(route.clone()),
            distance_meters: distance,
            ..test_support::segment()
        }
        .into();
        active.insert(&fixture.db).await.unwrap();
    }
    segment_summaries::ActiveModel {
        segment_id: Set(1),
        effort_count: Set(1),
        best_duration_seconds: Set(Some(60)),
        ..Default::default()
    }
    .insert(&fixture.db)
    .await
    .unwrap();
    segment_user_summaries::ActiveModel {
        segment_id: Set(1),
        user_id: Set(7),
        effort_count: Set(1),
        personal_best_duration_seconds: Set(Some(60)),
        ..Default::default()
    }
    .insert(&fixture.db)
    .await
    .unwrap();
    let active: segment_efforts::ActiveModel = segment_efforts::Model {
        end_route_point_index: end,
        ..test_support::segment_effort()
    }
    .into();
    active.insert(&fixture.db).await.unwrap();
    end
}

#[tokio::test]
async fn segment_handlers_preserve_owner_summary_and_ranked_effort_contracts() {
    let fixture = Fixture::new().await;
    seed_segment(&fixture).await;
    let list = fixture.json("GET", "/api/segments", json!({}), 200).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(
        (list[0]["id"].as_i64(), list[0]["effort_count"].as_i64()),
        (Some(1), Some(1))
    );
    let detail = fixture.json("GET", "/api/segments/1", json!({}), 200).await;
    assert_eq!(detail["current_user_pr_duration_seconds"], 60);
    fixture.json("GET", "/api/segments/2", json!({}), 404).await;
    let comparison = fixture
        .json("GET", "/api/segments/1/comparison", json!({}), 200)
        .await;
    assert_eq!(comparison["efforts"][0]["rider_name"], "Fixture customer");
    assert_eq!(comparison["efforts"][0]["activity_id"], 42);
    let bests = fixture
        .json("GET", "/api/segments/1/yearly-bests", json!({}), 200)
        .await;
    assert_eq!(bests["years"][0]["duration_seconds"], 60);
    let analysis = fixture
        .json(
            "GET",
            "/api/segments/1/effort-analysis?split_count=2",
            json!({}),
            200,
        )
        .await;
    assert_eq!(analysis["total_effort_count"], 1);
    assert_eq!(analysis["reference_effort"]["activity_id"], 42);
}

#[tokio::test]
async fn segment_updates_and_deletion_preserve_activity_and_foreign_segment() {
    let fixture = Fixture::new().await;
    seed_segment(&fixture).await;
    let updated = fixture
        .json(
            "PUT",
            "/api/segments/1",
            json!({"title":" Renamed loop ", "starred":true}),
            200,
        )
        .await;
    assert_eq!(updated["title"], "Renamed loop");
    assert_eq!(updated["starred"], true);
    let deleted = fixture
        .request("DELETE", "/api/segments/1", json!({}))
        .await;
    assert_eq!(deleted.status(), 204);
    assert!(segments::Model::find_by_id(&fixture.db, 1)
        .await
        .unwrap()
        .is_none());
    assert!(segments::Model::find_by_id(&fixture.db, 2)
        .await
        .unwrap()
        .is_some());
    assert!(activities::Model::find_owned(&fixture.db, 42, 7)
        .await
        .unwrap()
        .is_some());
    assert!(segment_efforts::Model::for_activity_ids(&fixture.db, &[42])
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn preference_handlers_create_then_update_the_owned_record() {
    let fixture = Fixture::new().await;
    let initial = fixture
        .json("GET", "/api/preferences", json!({}), 200)
        .await;
    assert_eq!(initial["unit_system"], "imperial");
    let saved = fixture
        .json(
            "PUT",
            "/api/preferences",
            json!({"unit_system":"imperial","estimated_ftp_watts":250}),
            200,
        )
        .await;
    assert_eq!(saved["unit_system"], "imperial");
    assert_eq!(saved["estimated_ftp_watts"], 250);
    let saved = fixture
        .json(
            "PUT",
            "/api/preferences",
            json!({"unit_system":"mixed","estimated_ftp_watts":275}),
            200,
        )
        .await;
    assert_eq!(saved["unit_system"], "mixed");
    assert_eq!(saved["estimated_ftp_watts"], 275);
    let stored = user_preferences::Model::for_user(&fixture.db, 7)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.estimated_ftp_watts, Some(275));
    assert!(user_preferences::Model::for_user(&fixture.db, 8)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn segment_builder_reuses_owned_duplicate_and_rejects_foreign_activity() {
    let fixture = Fixture::new().await;
    let end = seed_segment(&fixture).await;
    let payload = json!({"activity_id":42, "title":"Fixture segment",
        "start_route_point_index":0, "end_route_point_index":end});
    let segment = fixture
        .json("POST", "/api/segments/from-activity", payload, 200)
        .await;
    assert_eq!(segment["id"], 1);
    assert_eq!(segment["builder_source"]["activity_id"], 42);
    let list = fixture.json("GET", "/api/segments", json!({}), 200).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
    fixture
        .json(
            "POST",
            "/api/segments/from-activity",
            json!({
                "activity_id":43, "title":"Foreign", "start_route_point_index":0,
                "end_route_point_index":end
            }),
            404,
        )
        .await;
}
