//! Receipt and causal context carried across durable queue boundaries.
use crate::observability::{inject_current_trace_context, TraceContextCarrier};
use chrono::{DateTime, Utc};
use serde::ser::Error as _;
use serde::{Deserialize, Serialize};
use std::future::Future;
use uuid::Uuid;

const PAYLOAD_KEY: &str = "_pipeline";

tokio::task_local! {
    static CURRENT: Option<PipelineContext>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PipelineContext {
    pub run_id: String,
    pub pipeline_started_at: DateTime<Utc>,
    pub entrypoint: String,
    pub request_id: Option<String>,
    pub parent_task_id: Option<i32>,
    pub trace_context: Option<TraceContextCarrier>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gateway_history: Option<serde_json::Value>,
}

impl PipelineContext {
    pub fn received(entrypoint: impl Into<String>, request_id: Option<String>) -> Self {
        Self {
            run_id: Uuid::new_v4().to_string(),
            pipeline_started_at: Utc::now(),
            entrypoint: entrypoint.into(),
            request_id,
            parent_task_id: None,
            trace_context: inject_current_trace_context(),
            gateway_history: None,
        }
    }

    pub fn current() -> Option<Self> {
        CURRENT.try_with(Clone::clone).ok().flatten()
    }

    pub fn for_task(mut self, task_id: i32) -> Self {
        self.parent_task_id = Some(task_id);
        self
    }

    pub async fn scope<F: Future>(context: Option<Self>, future: F) -> F::Output {
        CURRENT.scope(context, future).await
    }

    pub fn from_payload(payload: &serde_json::Value) -> Result<Option<Self>, serde_json::Error> {
        payload
            .get(PAYLOAD_KEY)
            .map(|value| serde_json::from_value(value.clone()))
            .transpose()
    }

    /// Metadata stays beside the typed job's data, never inside its domain payload.
    pub fn attach(payload: &mut serde_json::Value) -> Result<(), serde_json::Error> {
        let Some(mut context) = Self::current() else {
            return Ok(());
        };
        context.trace_context = inject_current_trace_context().or(context.trace_context);
        let object = payload.as_object_mut().ok_or_else(|| {
            serde_json::Error::custom("Queued pipeline payload must be a JSON object")
        })?;
        object.insert(PAYLOAD_KEY.into(), serde_json::to_value(context)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn rejects_payloads_that_cannot_carry_pipeline_metadata() {
        PipelineContext::scope(Some(PipelineContext::received("upload", None)), async {
            let mut payload = json!("unsupported scalar job");
            assert!(PipelineContext::attach(&mut payload).is_err());
        })
        .await;
    }

    #[tokio::test]
    async fn handoffs_preserve_receipt_and_run_while_recording_the_new_parent() {
        let root = PipelineContext::received("upload", Some("request-1".into()));
        let expected = root.clone();
        PipelineContext::scope(Some(root.for_task(42)), async {
            let mut payload = json!({"type": "RebuildFitnessFreshness", "data": {"user_id": 7}});
            PipelineContext::attach(&mut payload).unwrap();
            let restored = PipelineContext::from_payload(&payload).unwrap().unwrap();
            assert_eq!(restored.run_id, expected.run_id);
            assert_eq!(restored.pipeline_started_at, expected.pipeline_started_at);
            assert_eq!(restored.parent_task_id, Some(42));
            assert_eq!(restored.request_id, expected.request_id);
            assert_eq!(payload["data"], json!({"user_id": 7}));
            PipelineContext::scope(Some(restored.for_task(43)), async {
                let mut child = json!({"data": {"segment_ids": [1, 2]}});
                PipelineContext::attach(&mut child).unwrap();
                let context = PipelineContext::from_payload(&child).unwrap().unwrap();
                assert_eq!(context.pipeline_started_at, expected.pipeline_started_at);
                assert_eq!(context.parent_task_id, Some(43));
            })
            .await;
        })
        .await;
        assert!(PipelineContext::current().is_none());
    }

    #[tokio::test]
    async fn rerun_replaces_old_origin_and_missing_legacy_evidence_stays_unknown() {
        let old = PipelineContext::received("upload", None);
        let mut payload = json!({"_pipeline": old, "data": {"user_id": 7}});
        let replay = PipelineContext::received("admin", Some("rerun-1".into()));
        let expected = replay.run_id.clone();
        PipelineContext::scope(Some(replay), async {
            PipelineContext::attach(&mut payload).unwrap();
        })
        .await;
        assert_eq!(
            PipelineContext::from_payload(&payload)
                .unwrap()
                .unwrap()
                .run_id,
            expected
        );
        assert!(PipelineContext::from_payload(&json!({"data": {}}))
            .unwrap()
            .is_none());
    }
}
