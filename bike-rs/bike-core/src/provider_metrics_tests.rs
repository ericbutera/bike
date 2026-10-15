use super::refresh_provider_rate_limit_metrics;
use crate::entities::provider_rate_limit_buckets;
use crate::test_support;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Schema, Set};

const PROVIDER: &str = "quota_snapshot_fixture";

async fn quota_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    db.execute(&schema.create_table_from_entity(provider_rate_limit_buckets::Entity))
        .await
        .unwrap();
    db
}

async fn insert_bucket(db: &DatabaseConnection, bucket: &str, reset_at: DateTime<Utc>) {
    test_support::insert(
        db,
        provider_rate_limit_buckets::ActiveModel {
            provider: Set(PROVIDER.to_owned()),
            bucket: Set(bucket.to_owned()),
            limit_count: Set(200),
            used_count: Set(17),
            reset_at: Set(reset_at),
            ..Default::default()
        },
    )
    .await;
}

fn quota_gauge(name: &str, bucket: &str) -> Option<i64> {
    prometheus::gather()
        .into_iter()
        .find(|family| family.name() == name)?
        .get_metric()
        .iter()
        .find(|metric| {
            metric
                .get_label()
                .iter()
                .any(|label| label.name() == "provider" && label.value() == PROVIDER)
                && metric
                    .get_label()
                    .iter()
                    .any(|label| label.name() == "bucket" && label.value() == bucket)
        })
        .map(|metric| metric.get_gauge().value() as i64)
}

#[tokio::test]
async fn quota_snapshot_survives_restart_without_provider_calls() {
    let db = quota_database().await;
    let deadline = Utc::now() + Duration::minutes(15);
    insert_bucket(&db, "current", deadline).await;
    insert_bucket(&db, "expired", Utc::now() - Duration::minutes(1)).await;
    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_used", "current"),
        None
    );

    refresh_provider_rate_limit_metrics(&db).await.unwrap();

    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_used", "current"),
        Some(17)
    );
    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_remaining", "current"),
        Some(183)
    );
    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_limit", "current"),
        Some(200)
    );
    assert_eq!(
        quota_gauge(
            "bike_rust_provider_rate_limit_reset_timestamp_seconds",
            "current"
        ),
        Some(deadline.timestamp())
    );
    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_used", "expired"),
        Some(0)
    );
    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_remaining", "expired"),
        Some(200)
    );
    assert_eq!(
        quota_gauge("bike_rust_provider_rate_limit_used", "missing"),
        None
    );
}

#[tokio::test]
async fn quota_snapshot_database_errors_are_propagated() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    assert!(refresh_provider_rate_limit_metrics(&db).await.is_err());
}
