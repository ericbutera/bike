use crate::storage::AppStorage;
use axum::{extract::State, http::StatusCode};
use bike_core::auth::entities::{auth_events, refresh_tokens, users};
use bike_core::entities::{activities, activity_imports, segments};
use bike_core::test_support;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{Database, DatabaseConnection, EntityTrait, Set};
use std::sync::Arc;
use uuid::Uuid;

async fn insert_auth_records(db: &DatabaseConnection, created_at: DateTime<Utc>, suffix: &str) {
    let pid = Uuid::new_v4();
    test_support::insert(
        db,
        users::ActiveModel {
            pid: Set(pid),
            email: Set(format!("metrics-{suffix}@example.com")),
            api_key: Set(format!("metrics-{suffix}")),
            name: Set("Metrics fixture".into()),
            disabled: Set(false),
            created_at: Set(created_at),
            updated_at: Set(created_at),
            is_admin: Set(None),
            email_verified_at: Set(None),
            ..bike_core::test_support::new_customer()
        },
    )
    .await;
    test_support::insert(
        db,
        refresh_tokens::ActiveModel {
            token: Set(format!("metrics-{suffix}")),
            user_pid: Set(pid),
            expires_at: Set((created_at + Duration::days(7)).timestamp()),
            created_at: Set(created_at),
        },
    )
    .await;
    for event in [
        auth_events::EventType::LoginFailed,
        auth_events::EventType::PasswordResetRequest,
    ] {
        test_support::insert(
            db,
            auth_events::ActiveModel {
                ts: Set(created_at),
                event_type: Set(event.to_string()),
                ..Default::default()
            },
        )
        .await;
    }
}

async fn scrape_text(db: DatabaseConnection) -> String {
    crate::metrics::init_metrics();
    let response = crate::metrics::metrics_route(State(Arc::new(AppStorage::for_test(db)))).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .unwrap();
    String::from_utf8(body.to_vec()).unwrap()
}

#[tokio::test]
async fn metrics_scrape_exports_retained_records_and_rolling_windows() {
    let _guard = crate::metrics::scrape_tests::SCRAPE_TEST_LOCK.lock().await;
    let db = crate::metrics::scrape_tests::metrics_database().await;
    let now = Utc::now();
    for (created_at, suffix) in [
        (now - Duration::days(45), "old"),
        (now - Duration::hours(1), "recent"),
    ] {
        let activity = insert_metric_activity(&db, created_at).await;
        insert_metric_activity_import(&db, activity.id, created_at).await;
        insert_metric_segment(&db, created_at).await;
        insert_auth_records(&db, created_at, suffix).await;
    }
    let text = scrape_text(db.clone()).await;
    for kind in ["activities", "activity_imports", "segments"] {
        assert!(text.contains(&format!(
            "bike_rust_database_records{{kind=\"{kind}\",window=\"all\"}} 2"
        )));
        assert!(text.contains(&format!(
            "bike_rust_database_records{{kind=\"{kind}\",window=\"30d\"}} 1"
        )));
    }
    assert!(text.contains("bike_rust_database_records{kind=\"users\",window=\"24h\"} 1"));
    assert!(text.contains("bike_rust_database_records{kind=\"refresh_tokens\",window=\"30d\"} 1"));
    for event in ["login_failed", "password_reset_request"] {
        assert!(text.contains(&format!(
            "bike_rust_auth_events_recent{{event=\"{event}\",window=\"30d\"}} 1"
        )));
    }
    // The snapshot follows stored records, including deletions; these are gauges.
    activities::Entity::delete_by_id(1).exec(&db).await.unwrap();
    let text = scrape_text(db).await;
    assert!(text.contains("bike_rust_database_records{kind=\"activities\",window=\"all\"} 1"));
}

#[tokio::test]
async fn missing_database_records_fail_the_metrics_scrape() {
    let _guard = crate::metrics::scrape_tests::SCRAPE_TEST_LOCK.lock().await;
    crate::metrics::init_metrics();
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let response = crate::metrics::metrics_route(State(Arc::new(AppStorage::for_test(db)))).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

async fn insert_metric_activity(
    db: &DatabaseConnection,
    created_at: DateTime<Utc>,
) -> activities::Model {
    let activity = test_support::insert(
        db,
        activities::ActiveModel {
            user_id: Set(1),
            title: Set("Lunch Ride".to_string()),
            format: Set(Some("gpx".to_string())),
            started_at: Set(created_at - Duration::hours(2)),
            ended_at: Set(Some(created_at - Duration::hours(1))),
            distance_meters: Set(Some(25_000.0)),
            moving_time_seconds: Set(Some(3600)),
            total_time_seconds: Set(Some(3900)),
            elevation_gain_meters: Set(Some(400.0)),
            elevation_loss_meters: Set(Some(400.0)),
            average_speed_mps: Set(Some(7.0)),
            max_speed_mps: Set(Some(12.0)),
            average_heart_rate_bpm: Set(Some(140)),
            max_heart_rate_bpm: Set(Some(170)),
            average_cadence_rpm: Set(Some(85)),
            max_cadence_rpm: Set(Some(102)),
            calories: Set(Some(700)),
            created_at: Set(created_at),
            ..bike_core::test_support::new_activity()
        },
    )
    .await;
    test_support::update(
        db,
        activities::ActiveModel {
            id: Set(activity.id),
            created_at: Set(created_at),
            ..Default::default()
        },
    )
    .await
}

async fn insert_metric_activity_import(
    db: &DatabaseConnection,
    activity_id: i32,
    created_at: DateTime<Utc>,
) {
    let import = test_support::insert(
        db,
        activity_imports::ActiveModel {
            user_id: Set(1),
            status: Set("processed".to_string()),
            activity_id: Set(Some(activity_id)),
            processing_stage: Set("complete".to_string()),
            processed_at: Set(Some(created_at)),
            last_processing_event_at: Set(Some(created_at)),
            original_filename: Set("lunch-ride.gpx".to_string()),
            storage_path: Set("/tmp/lunch-ride.gpx".to_string()),
            size_bytes: Set(1_024),
            created_at: Set(created_at),
            ..bike_core::test_support::new_activity_import()
        },
    )
    .await;
    test_support::update(
        db,
        activity_imports::ActiveModel {
            id: Set(import.id),
            created_at: Set(created_at),
            ..Default::default()
        },
    )
    .await;
}

async fn insert_metric_segment(db: &DatabaseConnection, created_at: DateTime<Utc>) {
    let segment = test_support::insert(
        db,
        segments::ActiveModel {
            user_id: Set(1),
            title: Set("North Climb".to_string()),
            source: Set("manual_segment_import".to_string()),
            mode: Set("xc".to_string()),
            starred: Set(false),
            original_filename: Set(Some("north-climb.gpx".to_string())),
            format: Set(Some("gpx".to_string())),
            distance_meters: Set(Some(1800.0)),
            route_data_json: Set(None),
            last_activity_change_at: Set(created_at),
            created_at: Set(created_at),
            ..Default::default()
        },
    )
    .await;
    test_support::update(
        db,
        segments::ActiveModel {
            id: Set(segment.id),
            created_at: Set(created_at),
            ..Default::default()
        },
    )
    .await;
}
