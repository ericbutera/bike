use super::{init_metrics, metrics_route};
use crate::storage::AppStorage;
use axum::{extract::State, http::StatusCode};
use bike_core::auth::entities::{auth_events, refresh_tokens, users};
use bike_core::entities::{
    activities, activity_import_attempts, activity_imports, provider_rate_limit_buckets, segments,
    strava_connections,
};
use bike_core::test_support;
use chrono::{Duration, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Schema, Set};
use std::sync::Arc;

pub(crate) static SCRAPE_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) async fn metrics_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        schema.create_table_from_entity(activities::Entity),
        schema.create_table_from_entity(activity_imports::Entity),
        schema.create_table_from_entity(activity_import_attempts::Entity),
        schema.create_table_from_entity(segments::Entity),
        schema.create_table_from_entity(users::Entity),
        schema.create_table_from_entity(refresh_tokens::Entity),
        schema.create_table_from_entity(auth_events::Entity),
        schema.create_table_from_entity(provider_rate_limit_buckets::Entity),
        schema.create_table_from_entity(strava_connections::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    db
}

#[tokio::test]
async fn metrics_response_reloads_quota_without_provider_requests() {
    let _guard = SCRAPE_TEST_LOCK.lock().await;
    let db = metrics_database().await;
    for (bucket, deadline) in [
        ("current", Utc::now() + Duration::minutes(15)),
        ("expired", Utc::now() - Duration::minutes(1)),
    ] {
        test_support::insert(
            &db,
            provider_rate_limit_buckets::ActiveModel {
                provider: Set("quota_scrape_fixture".into()),
                bucket: Set(bucket.into()),
                limit_count: Set(200),
                used_count: Set(17),
                reset_at: Set(deadline),
                ..Default::default()
            },
        )
        .await;
    }
    init_metrics();
    let response = metrics_route(State(Arc::new(AppStorage::for_test(db)))).await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), 1_048_576)
        .await
        .unwrap();
    let body = std::str::from_utf8(&bytes).unwrap();
    for (name, bucket, value) in [
        ("used", "current", 17),
        ("remaining", "current", 183),
        ("used", "expired", 0),
        ("remaining", "expired", 200),
    ] {
        assert!(
            body.lines().any(|line| {
                line.starts_with(&format!("bike_rust_provider_rate_limit_{name}{{"))
                    && line.contains("provider=\"quota_scrape_fixture\"")
                    && line.contains(&format!("bucket=\"{bucket}\""))
                    && line.ends_with(&format!(" {value}"))
            }),
            "missing quota {bucket}/{name}: {body}"
        );
    }
}

#[tokio::test]
async fn metrics_response_rejects_a_failed_quota_snapshot() {
    let _guard = SCRAPE_TEST_LOCK.lock().await;
    init_metrics();
    let db = metrics_database().await;
    let storage = Arc::new(AppStorage::for_test(db.clone()));
    let ready = metrics_route(State(storage.clone())).await;
    assert_eq!(ready.status(), StatusCode::OK);
    db.execute_unprepared("DROP TABLE provider_rate_limit_buckets")
        .await
        .unwrap();
    let response = metrics_route(State(storage)).await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
