//! Durable event acceptance, before Temporal workflow execution begins.
use bike_core::entities::strava_events::HandoffBacklog;
use chrono::{DateTime, Utc};
use prometheus::{IntGauge, Registry};
use sea_orm::{ConnectionTrait, DbErr};

pub struct HandoffMetrics {
    pending: IntGauge,
    oldest: IntGauge,
    sampled_at: IntGauge,
}

impl HandoffMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(Self {
            pending: gauge(
                registry,
                "bike_rust_strava_handoff_pending_events",
                "Strava events not yet submitted to Temporal.",
            )?,
            oldest: gauge(
                registry,
                "bike_rust_strava_handoff_oldest_accepted_timestamp_seconds",
                "Acceptance timestamp of the oldest pending Strava event; zero when empty.",
            )?,
            sampled_at: gauge(
                registry,
                "bike_rust_strava_handoff_last_sample_timestamp_seconds",
                "Timestamp of the last successful durable handoff backlog sample; zero before the first sample.",
            )?,
        })
    }

    pub async fn sample(&self, db: &impl ConnectionTrait, now: DateTime<Utc>) -> Result<(), DbErr> {
        let backlog = HandoffBacklog::load(db).await?;
        self.pending.set(backlog.pending);
        self.oldest
            .set(backlog.oldest_accepted_at.map_or(0, |at| at.timestamp()));
        self.sampled_at.set(now.timestamp());
        Ok(())
    }
}

fn gauge(registry: &Registry, name: &str, help: &str) -> Result<IntGauge, prometheus::Error> {
    let metric = IntGauge::new(name, help)?;
    registry.register(Box::new(metric.clone()))?;
    Ok(metric)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{DatabaseBackend, MockDatabase, Value};
    use std::collections::BTreeMap;

    fn backlog(pending: i64, oldest: Option<DateTime<Utc>>) -> BTreeMap<String, Value> {
        [
            ("pending".into(), pending.into()),
            ("oldest_accepted_at".into(), oldest.into()),
        ]
        .into()
    }

    #[tokio::test]
    async fn samples_all_unsubmitted_events_including_delayed_retries_then_clears_the_gauges() {
        let registry = Registry::new();
        let metrics = HandoffMetrics::new(&registry).unwrap();
        let now = DateTime::from_timestamp(1790856000, 0).unwrap();
        let oldest = now - chrono::Duration::minutes(10);
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![backlog(123, Some(oldest))], vec![backlog(0, None)]])
            .into_connection();
        metrics.sample(&db, now).await.unwrap();
        assert_eq!(metrics.pending.get(), 123);
        assert_eq!(metrics.oldest.get(), oldest.timestamp());
        assert_eq!(metrics.sampled_at.get(), now.timestamp());
        metrics.sample(&db, now + chrono::Duration::seconds(30)).await.unwrap();
        assert_eq!(metrics.pending.get(), 0);
        assert_eq!(metrics.oldest.get(), 0);
        assert_eq!(registry.gather().len(), 3);
        assert!(registry
            .gather()
            .iter()
            .all(|family| family.get_metric()[0].get_label().is_empty()));
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("submitted_at IS NULL"));
        assert!(!log.contains("next_attempt_at"));
    }

    #[tokio::test]
    async fn failed_sampling_preserves_last_values_and_does_not_claim_freshness() {
        let metrics = HandoffMetrics::new(&Registry::new()).unwrap();
        let now = DateTime::from_timestamp(1790856000, 0).unwrap();
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![backlog(2, Some(now))]])
            .append_query_errors([DbErr::Custom("fixture database unavailable".into())])
            .into_connection();
        metrics.sample(&db, now).await.unwrap();
        assert!(metrics.sample(&db, now + chrono::Duration::seconds(30)).await.is_err());
        assert_eq!(metrics.pending.get(), 2);
        assert_eq!(metrics.sampled_at.get(), now.timestamp());
    }

    #[tokio::test]
    async fn missing_aggregate_and_registration_failure_are_reported() {
        let registry = Registry::new();
        let metrics = HandoffMetrics::new(&registry).unwrap();
        assert!(HandoffMetrics::new(&registry).is_err());
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([Vec::<BTreeMap<String, Value>>::new()])
            .into_connection();
        assert!(metrics.sample(&db, Utc::now()).await.is_err());
        assert_eq!(metrics.sampled_at.get(), 0);
    }
}
