use super::fixture::Fixture;
use crate::activities::StravaActivities;
use bike_core::strava::{
    events::StravaOperation,
    logging::{AttemptStatus, ExchangeContext, TemporalIds},
};
use std::{
    io::Write,
    sync::{Arc, Mutex, OnceLock},
};
use tracing_subscriber::layer::SubscriberExt;

#[derive(Clone)]
pub(super) struct Logs(Arc<Mutex<Vec<u8>>>);

impl Default for Logs {
    fn default() -> Self {
        // Keep dynamic callsite interest alive while parallel tests swap local subscribers.
        static DISPATCH: OnceLock<tracing::Dispatch> = OnceLock::new();
        DISPATCH.get_or_init(|| {
            tracing::Dispatch::new(
                tracing_subscriber::registry().with(tracing_subscriber::filter::dynamic_filter_fn(|_, _| false)),
            )
        });
        Self(Arc::default())
    }
}
impl Write for Logs {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Logs {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

impl Logs {
    pub fn rows(&self) -> Vec<serde_json::Value> {
        let text = String::from_utf8(self.0.lock().unwrap().clone()).unwrap();
        text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    pub fn capture(&self, operation: impl FnOnce()) {
        opentelemetry::global::set_text_map_propagator(opentelemetry_sdk::propagation::TraceContextPropagator::new());
        let subscriber = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_writer(self.clone())
                    .with_ansi(false),
            )
            .with(tracing_subscriber::filter::dynamic_filter_fn(|metadata, _| {
                *metadata.level() <= tracing::Level::INFO
            }));
        tracing::subscriber::with_default(subscriber, operation);
    }

    pub fn workflow(&self, id: &str, status: &str) -> serde_json::Value {
        self.rows()
            .into_iter()
            .find(|row| row["span"]["temporal_workflow_id"] == id && row["fields"]["status"] == status)
            .unwrap_or_else(|| panic!("Expected {id} {status} lifecycle log: {:?}", self.rows()))
    }
}

#[test]
fn handoff_success_and_failure_share_searchable_customer_remote_and_event_fields() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let row = stored_event(&fixture);
    let config = crate::client::TemporalConfig {
        address: "http://fixture.invalid".into(),
        namespace: "bike".into(),
        task_queue: "bike-strava".into(),
    };
    let logs = Logs::default();
    logs.capture(|| {
        crate::client::handoff_span(&row, &config).in_scope(|| {
            tracing::info!(temporal_run_id = "fixture-run", "Strava event handed to Temporal");
            tracing::error!(error = "fixture error", "Temporal handoff will retry");
        });
    });
    let rows = logs.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["level"], "INFO");
    assert_eq!(rows[1]["level"], "ERROR");
    for entry in rows {
        let fields = &entry["span"];
        assert_eq!(fields["bike_customer_id"], row.customer_id.to_string());
        assert_eq!(fields["bike_user_id"], row.user_id);
        assert_eq!(fields["strava_athlete_id"], row.athlete_id);
        assert_eq!(fields["strava_activity_id"], row.activity_id.unwrap());
        assert_eq!(fields["strava_event_id"], row.id);
        assert_eq!(fields["correlation_id"], row.correlation_id);
        assert_eq!(fields["temporal_namespace"], config.namespace);
        assert_eq!(fields["temporal_workflow_id"], row.workflow_id);
    }
}

fn stored_event(fixture: &Fixture) -> bike_core::entities::strava_events::Model {
    use bike_core::strava::native::ActivityStatus;
    let event = &fixture.event;
    bike_core::entities::strava_events::Model {
        id: event.strava_event_id.to_string(),
        event_key: "fixture".into(),
        correlation_id: event.correlation_id.to_string(),
        parent_event_id: None,
        user_id: event.bike_user_id,
        customer_id: event.bike_customer_id,
        athlete_id: event.strava_athlete_id,
        activity_id: Some(fixture.activity_id),
        workflow_id: event.workflow_id(),
        event: serde_json::to_value(event).unwrap(),
        status: ActivityStatus::Pending,
        mutation: None,
        accepted_at: event.connection_generation,
        submitted_at: None,
        next_attempt_at: event.connection_generation,
        handoff_error: None,
    }
}

#[test]
fn structured_attempt_and_exchange_logs_include_customer_remote_and_temporal_ids() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut event = fixture.event.clone();
    event.bike_activity_id = Some(42);
    event.parent_event_id = Some(uuid::Uuid::from_u128(9));
    let context = ExchangeContext {
        event,
        temporal: TemporalIds {
            temporal_namespace: "bike".into(),
            temporal_workflow_id: "fixture-workflow".into(),
            temporal_run_id: "fixture-run".into(),
            temporal_activity_id: "fixture-activity".into(),
            temporal_attempt: 2,
        },
    };
    let logs = Logs::default();
    logs.capture(|| {
        context.attempt(StravaActivities::fetch_activity.name(), AttemptStatus::Started);
        context.exchange(
            "fixture-exchange",
            "GET",
            "/api/v3/activities/99133701",
            Some(200),
            Some("fixture-strava-request"),
        );
    });
    let rows = logs.rows();
    assert_eq!(rows.len(), 2);
    for row in &rows {
        let fields = &row["fields"];
        assert_event_fields(fields, &context.event);
        assert_temporal_fields(fields, &context.temporal);
    }
    assert_eq!(rows[1]["fields"]["exchange_id"], "fixture-exchange");
    assert_eq!(rows[1]["fields"]["strava_request_id"], "fixture-strava-request");
    assert_eq!(rows[1]["fields"]["status"], 200);
}

fn assert_event_fields(fields: &serde_json::Value, event: &bike_core::strava::events::StravaEvent) {
    assert_eq!(fields["bike_customer_id"], event.bike_customer_id.to_string());
    assert_eq!(fields["bike_user_id"], event.bike_user_id);
    assert_eq!(
        fields["bike_activity_id"].as_i64(),
        event.bike_activity_id.map(i64::from)
    );
    assert_eq!(fields["parent_event_id"], event.parent_event_id.unwrap().to_string());
    assert_eq!(fields["request_id"].as_str(), event.request_id.as_deref());
    assert_eq!(fields["trace_id"], "12345678901234567890123456789012");
    assert_eq!(fields["strava_athlete_id"], event.strava_athlete_id);
    assert_eq!(fields["strava_activity_id"].as_i64(), event.strava_activity_id);
    assert_eq!(fields["strava_event_id"], event.strava_event_id.to_string());
    assert_eq!(fields["correlation_id"], event.correlation_id.to_string());
}

fn assert_temporal_fields(fields: &serde_json::Value, ids: &TemporalIds) {
    assert_eq!(fields["temporal_namespace"], ids.temporal_namespace);
    assert_eq!(fields["temporal_workflow_id"], ids.temporal_workflow_id);
    assert_eq!(fields["temporal_run_id"], ids.temporal_run_id);
    assert_eq!(fields["temporal_activity_id"], ids.temporal_activity_id);
    assert_eq!(fields["temporal_attempt"], ids.temporal_attempt);
}
