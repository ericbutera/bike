use super::{competition, prepare_heatmaps, seed, Dataset};
use axum::{body::to_bytes, body::Body, http::Request, Router};
use bike_core::auth::entities::users;
use bike_core::entities::activities;
use bike_core::heatmaps::{data::HeatmapData, types::HeatmapQuery};
use bike_core::test_support as fixtures;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, Set};
use serde_json::{json, Value};
use std::{collections::BTreeSet, process::Command, sync::Arc};
use tower::ServiceExt;

#[tokio::test]
#[ignore = "Set BIKE_SEGMENT_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn staging_competition_populates_segment_screens_and_preserves_reloads() {
    let uploads = tempfile::tempdir().unwrap();
    std::env::set_var("UPLOADS_DIR", uploads.path());
    let (db, url, schema) = database().await;
    let mut dataset = staging_export();
    let competition = dataset.competition.take().unwrap();
    let riders: BTreeSet<_> = competition.rides.iter().map(|ride| ride.user_id).collect();
    assert_eq!(riders.len(), 15);
    let ride_owners: Vec<_> = competition.rides.iter().map(|ride| ride.user_id).collect();

    seed(&db, dataset).await.unwrap();
    fixtures::update(
        &db,
        activities::ActiveModel {
            id: Set(1109),
            title: Set("Preserve reviewer edit".into()),
            ..Default::default()
        },
    )
    .await;
    competition::seed(&db, competition).await.unwrap();
    prepare_heatmaps(&db).await.unwrap();

    let app = application(&db, &url).await;
    for segment_id in 4101..=4105 {
        assert_segment_screens(&app, segment_id, &riders).await;
    }
    assert_personal_reports(&app).await;
    for rider in riders {
        let progress = HeatmapData::progress(&db, rider, &HeatmapQuery::default())
            .await
            .unwrap();
        let baseline = if rider == 1 { 2 } else { 0 };
        let expected = ride_owners.iter().filter(|owner| **owner == rider).count() + baseline;
        assert_eq!(progress.ready as usize, expected, "rider {rider}");
        assert_eq!(progress.pending, 0);
    }

    let before = get(&app, "/api/segments/4101/comparison").await;
    competition::seed(&db, staging_export().competition.unwrap())
        .await
        .unwrap();
    assert_eq!(get(&app, "/api/segments/4101/comparison").await, before);
    assert_eq!(
        get(&app, "/api/activities/1109").await["title"],
        "Preserve reviewer edit"
    );
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

fn staging_export() -> Dataset {
    // Use the same scenario definition as staging, rather than another set of
    // riders, ranks or hand-written database rows in this test.
    let output = Command::new("node")
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .args([
            "--input-type=module",
            "-e",
            "import {stagingData} from './bike-ui/tests/e2e/helpers/seeds.mjs'; process.stdout.write(JSON.stringify(stagingData('reviewer@example.test', id => `fixture-key-${id}`)));",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    serde_json::from_slice(&output.stdout).unwrap()
}

async fn database() -> (DatabaseConnection, String, String) {
    let url = std::env::var("BIKE_SEGMENT_TEST_DATABASE_URL").expect("disposable PostgreSQL URL");
    let root = bike_core::db::connect_database(&url).await.unwrap();
    let schema = format!("segment_fixture_{}", uuid::Uuid::new_v4().simple());
    root.execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut scoped = reqwest::Url::parse(&url).unwrap();
    scoped
        .query_pairs_mut()
        .append_pair("options", &format!("-c search_path={schema}"));
    let url = scoped.to_string();
    let db = bike_core::db::connect_database(&url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();
    (db, url, schema)
}

async fn application(db: &DatabaseConnection, url: &str) -> Router {
    let owner = users::Entity::find_by_id(1).one(db).await.unwrap().unwrap();
    let mut state = api::storage::AppStorage::new(url).await;
    state.local_admin_user_pid = Some(owner.pid);
    api::controllers::routes().with_state(Arc::new(state))
}

async fn get(app: &Router, path: &str) -> Value {
    let response = app
        .clone()
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 10_000_000).await.unwrap();
    assert_eq!(status, 200, "{path}: {}", String::from_utf8_lossy(&body));
    serde_json::from_slice(&body).unwrap()
}

async fn assert_segment_screens(app: &Router, segment_id: i32, riders: &BTreeSet<i32>) {
    let detail = get(app, &format!("/api/segments/{segment_id}")).await;
    let comparison = get(app, &format!("/api/segments/{segment_id}/comparison")).await;
    let efforts = comparison["efforts"].as_array().unwrap();
    let actual_riders: BTreeSet<_> = efforts
        .iter()
        .map(|effort| effort["rider_user_id"].as_i64().unwrap() as i32)
        .collect();
    assert_eq!(&actual_riders, riders, "segment {segment_id}");
    assert_eq!(detail["effort_count"], json!(efforts.len()));
    assert_eq!(detail["best_duration_seconds"], efforts[0]["duration_seconds"]);
    assert_eq!(efforts[0]["rider_user_id"], if segment_id <= 4103 { 102 } else { 101 });
    assert!(!comparison["route_points"].as_array().unwrap().is_empty());
    for effort in efforts {
        assert!(effort["activity_title"].as_str().unwrap().starts_with("Fixture"));
        assert!(!effort["rider_name"].as_str().unwrap().is_empty());
        assert_eq!(effort["route_points"][0]["elapsed_seconds"], 0);
    }
    if segment_id <= 4103 {
        let personal_best = efforts.iter().find(|effort| effort["activity_id"] == 31001).unwrap();
        assert_eq!(
            detail["current_user_pr_duration_seconds"],
            personal_best["duration_seconds"]
        );
        let attempts: Vec<_> = efforts.iter().filter(|effort| effort["activity_id"] == 31005).collect();
        let indices: BTreeSet<_> = attempts
            .iter()
            .map(|effort| effort["effort_index"].as_i64().unwrap())
            .collect();
        assert!(attempts.len() >= 3);
        assert_eq!(indices, (1..=attempts.len() as i64).collect());
        if segment_id == 4101 {
            // The source City Loop crosses Breakin The Law twice per repeat.
            assert_eq!(attempts.len(), 6);
        }
    }
}

async fn assert_personal_reports(app: &Router) {
    let progress = get(app, "/api/segments/4101/yearly-bests").await;
    let years: Vec<_> = progress["years"]
        .as_array()
        .unwrap()
        .iter()
        .map(|year| (year["year"].as_i64().unwrap(), year["activity_id"].as_i64().unwrap()))
        .collect();
    assert_eq!(years, [(2024, 31109), (2025, 31001), (2026, 31004)]);
    let reference = progress["years"][1]["effort_id"].as_i64().unwrap();
    let analysis = get(
        app,
        &format!("/api/segments/4101/effort-analysis?reference_effort_id={reference}&split_count=5"),
    )
    .await;
    assert_eq!(analysis["reference_effort"]["activity_id"], 31001);
    assert_eq!(analysis["sections"].as_array().unwrap().len(), 5);
    let attempts = analysis["efforts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|effort| effort["activity_id"] == 31005)
        .count();
    assert_eq!(attempts, 6);
}
