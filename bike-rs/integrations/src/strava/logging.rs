//! Correlation fields for native Temporal activities and provider exchanges.
use super::events::StravaEvent;
use serde::{Deserialize, Serialize};
use std::future::Future;
use tracing::Instrument;

tokio::task_local! { static CURRENT: ExchangeContext; }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TemporalIds {
    pub temporal_namespace: String,
    pub temporal_workflow_id: String,
    pub temporal_run_id: String,
    pub temporal_activity_id: String,
    pub temporal_attempt: u32,
}

#[derive(Clone, Debug)]
pub struct ExchangeContext {
    pub event: StravaEvent,
    pub temporal: TemporalIds,
}

#[derive(Clone, Copy, Debug)]
pub enum AttemptStatus {
    Started,
    Completed,
    Failed,
    Cancelled,
}

impl AttemptStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderResponseStatus {
    Reused,
    Fetched,
    TransportError,
}

impl ProviderResponseStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reused => "reused",
            Self::Fetched => "fetched",
            Self::TransportError => "transport_error",
        }
    }
}

impl ExchangeContext {
    pub fn attempt(&self, processor: &str, status: AttemptStatus) {
        let event = &self.event;
        let temporal = &self.temporal;
        tracing::info!(processor, status = status.as_str(), correlation_id = %event.correlation_id,
            parent_event_id = event.parent_event_id.map(|id| id.to_string()).as_deref(),
            request_id = event.request_id.as_deref(), trace_id = event.trace_id().as_deref(),
            strava_event_id = %event.strava_event_id, bike_customer_id = %event.bike_customer_id,
            bike_user_id = event.bike_user_id, bike_activity_id = event.bike_activity_id,
            strava_athlete_id = event.strava_athlete_id, strava_activity_id = event.strava_activity_id,
            temporal_namespace = temporal.temporal_namespace, temporal_workflow_id = temporal.temporal_workflow_id,
            temporal_run_id = temporal.temporal_run_id, temporal_activity_id = temporal.temporal_activity_id,
            temporal_attempt = temporal.temporal_attempt, "Strava activity attempt");
    }
    pub fn current() -> Option<Self> {
        CURRENT.try_with(Clone::clone).ok()
    }

    pub fn span(&self, processor: &str) -> tracing::Span {
        let event = &self.event;
        let temporal = &self.temporal;
        let activity_id = event.strava_activity_id.map(|id| id.to_string());
        let span = tracing::info_span!("strava.activity",
            otel.name = processor,
            bike.operation = processor,
            processor,
            correlation_id = %event.correlation_id,
            strava_event_id = %event.strava_event_id,
            parent_event_id = event.parent_event_id.map(|id| id.to_string()).as_deref(),
            bike_customer_id = %event.bike_customer_id,
            bike_user_id = event.bike_user_id,
            bike_activity_id = event.bike_activity_id,
            strava_athlete_id = %event.strava_athlete_id,
            strava_activity_id = activity_id.as_deref(),
            connection_id = event.connection_id,
            strava_event_time = event.event_time,
            temporal_namespace = temporal.temporal_namespace,
            temporal_workflow_id = temporal.temporal_workflow_id,
            temporal_run_id = temporal.temporal_run_id,
            temporal_activity_id = temporal.temporal_activity_id,
            temporal_attempt = temporal.temporal_attempt,
            request_id = event.request_id.as_deref(),
            trace_id = event.trace_id().as_deref(), span_id = tracing::field::Empty);
        if let Some(carrier) = &event.trace_context {
            bike_core::observability::set_span_parent_from_carrier(&span, Some(carrier));
        }
        bike_core::observability::record_span_trace_context(&span);
        span
    }

    pub async fn scope<F: Future>(self, processor: &str, future: F) -> F::Output {
        let span = self.span(processor);
        CURRENT.scope(self, future.instrument(span)).await
    }

    pub fn exchange(
        &self,
        exchange_id: &str,
        method: &str,
        path: &str,
        status: Option<u16>,
        strava_request_id: Option<&str>,
    ) {
        let event = &self.event;
        let temporal = &self.temporal;
        tracing::info!(
            correlation_id = %event.correlation_id,
            parent_event_id = event.parent_event_id.map(|id| id.to_string()).as_deref(),
            request_id = event.request_id.as_deref(), trace_id = event.trace_id().as_deref(),
            strava_event_id = %event.strava_event_id,
            bike_customer_id = %event.bike_customer_id,
            bike_user_id = event.bike_user_id,
            bike_activity_id = event.bike_activity_id,
            strava_athlete_id = event.strava_athlete_id,
            strava_activity_id = event.strava_activity_id,
            temporal_namespace = temporal.temporal_namespace,
            temporal_workflow_id = temporal.temporal_workflow_id,
            temporal_run_id = temporal.temporal_run_id,
            temporal_activity_id = temporal.temporal_activity_id,
            temporal_attempt = temporal.temporal_attempt,
            exchange_id, method, path, status, strava_request_id,
            "Strava API exchange");
    }
}
