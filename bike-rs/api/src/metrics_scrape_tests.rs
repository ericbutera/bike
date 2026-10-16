use crate::{
    metrics::{init_metrics, metrics_route},
    storage::AppStorage,
};
use axum::{body::to_bytes, extract::State};
use bike_core::{entities::provider_rate_limit_buckets, test_support as fixtures};
use sea_orm::{DatabaseConnection, DbErr, MockDatabase};
use std::sync::Arc;

static SCRAPE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn record_counts() -> MockDatabase {
    fixtures::postgres_mock().append_query_results((0..10).map(|_| vec![fixtures::count_row(5)]))
}

async fn scrape(db: DatabaseConnection) -> (u16, String) {
    let uploads = tempfile::tempdir().unwrap();
    let mut state = AppStorage::for_test(db);
    state.uploads_dir = uploads.path().to_str().unwrap().into();
    init_metrics();
    let response = metrics_route(State(Arc::new(state))).await;
    let status = response.status().as_u16();
    let bytes = to_bytes(response.into_body(), 1_048_576).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

#[tokio::test]
async fn scrape_exports_retained_counts_and_current_or_expired_provider_quota() {
    let _guard = SCRAPE_LOCK.lock().await;
    let now = chrono::Utc::now();
    let buckets = [
        provider_rate_limit_buckets::Model {
            reset_at: now + chrono::Duration::minutes(15),
            ..fixtures::quota_bucket()
        },
        provider_rate_limit_buckets::Model {
            id: 2,
            bucket: "expired".into(),
            reset_at: now - chrono::Duration::minutes(1),
            ..fixtures::quota_bucket()
        },
    ];
    let db = record_counts()
        .append_query_results([buckets.to_vec()])
        .append_query_results([vec![fixtures::count_row(2)], vec![fixtures::count_row(1)]])
        .into_connection();
    let (status, text) = scrape(db.clone()).await;
    assert_eq!(status, 200);
    for metric in [
        "bike_rust_database_records{kind=\"activities\",window=\"all\"} 5",
        "bike_rust_database_records{kind=\"activity_imports\",window=\"30d\"} 5",
        "bike_rust_database_records{kind=\"segments\",window=\"all\"} 5",
        "bike_rust_strava_connected_athletes 2",
        "bike_rust_api_failed_activity_imports 1",
    ] {
        assert!(text.contains(metric), "missing {metric}");
    }
    for (kind, bucket, expected) in [
        ("used", "current", 17),
        ("remaining", "current", 183),
        ("used", "expired", 0),
        ("remaining", "expired", 200),
    ] {
        assert!(
            text.lines()
                .filter(
                    |line| line.starts_with(&format!("bike_rust_provider_rate_limit_{kind}{{"))
                        && line.contains(&format!("bucket=\"{bucket}\""))
                )
                .any(|line| line.ends_with(&format!(" {expected}"))),
            "missing {kind}/{bucket}"
        );
    }
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("created_at") && log.contains("COUNT"));
}

#[tokio::test]
async fn failed_record_snapshot_returns_service_unavailable() {
    let _guard = SCRAPE_LOCK.lock().await;
    let db = fixtures::postgres_mock()
        .append_query_errors([DbErr::Custom("fixture records unavailable".into())])
        .into_connection();
    assert_eq!(scrape(db).await.0, 503);
}

#[tokio::test]
async fn failed_quota_snapshot_returns_service_unavailable() {
    let _guard = SCRAPE_LOCK.lock().await;
    let db = record_counts()
        .append_query_errors([DbErr::Custom("fixture quota unavailable".into())])
        .into_connection();
    assert_eq!(scrape(db).await.0, 503);
}
