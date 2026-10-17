use bike_core::platform::aggregator::StoredRecordCounts;
use bike_core::platform::system_metrics::SystemMetrics;
use once_cell::sync::Lazy;
use prometheus::{register_int_gauge_vec, IntGaugeVec};
use sea_orm::{DatabaseConnection, DbErr};

static STORED_RECORDS: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_rust_database_records",
        "Retained database records at scrape time by bounded kind and rolling creation window.",
        &["kind", "window"]
    )
    .expect("register stored record counts")
});

static RECENT_AUTH_EVENTS: Lazy<IntGaugeVec> = Lazy::new(|| {
    register_int_gauge_vec!(
        "bike_rust_auth_events_recent",
        "Retained authentication events in the last 30 days, including legacy password auth.",
        &["event", "window"]
    )
    .expect("register retained auth event counts")
});

pub fn init() {
    Lazy::force(&STORED_RECORDS);
    Lazy::force(&RECENT_AUTH_EVENTS);
}

pub async fn refresh(db: &DatabaseConnection) -> Result<(), DbErr> {
    let snapshot = SystemMetrics::collect(db).await?;
    set_record_counts("activities", &snapshot.activities);
    set_record_counts("activity_imports", &snapshot.activity_imports);
    set_record_counts("segments", &snapshot.segments);
    set_record_count("users", "24h", snapshot.auth.new_signups_last_24h);
    set_record_count("refresh_tokens", "30d", snapshot.auth.refresh_tokens_created_last_30d);
    for (event, count) in [
        ("login_failed", snapshot.auth.failed_logins_last_30d),
        ("password_reset_request", snapshot.auth.password_resets_last_30d),
    ] {
        RECENT_AUTH_EVENTS
            .with_label_values(&[event, "30d"])
            .set(gauge_value(count));
    }
    Ok(())
}

fn set_record_counts(kind: &str, counts: &StoredRecordCounts) {
    set_record_count(kind, "all", counts.total);
    set_record_count(kind, "30d", counts.created_last_30d);
}

fn set_record_count(kind: &str, window: &str, count: u64) {
    STORED_RECORDS
        .with_label_values(&[kind, window])
        .set(gauge_value(count));
}

fn gauge_value(count: u64) -> i64 {
    count.min(i64::MAX as u64) as i64
}
