use super::*;
use bike_core::integration_events_service::{record_event, NewIntegrationEvent};
use serde_json::{json, Value};

pub(super) struct TaskRecord {
    user_id: i32,
    activity_id: Option<i32>,
    provider: &'static str,
    action: Value,
    command_id: Uuid,
    actor_id: i32,
    request_id: Option<String>,
}

impl TaskRecord {
    pub(super) fn activity(
        context: &RequestContext,
        activity: &activities::Model,
        request: &ActivityTaskRequest,
    ) -> Self {
        Self {
            user_id: activity.user_id,
            activity_id: Some(activity.id),
            provider: if matches!(request.task, ActivityAction::StravaResync) {
                "strava"
            } else {
                "activity_processing"
            },
            action: json!({ "task": request.task }),
            command_id: request.command_id,
            actor_id: context.actor_id,
            request_id: context.request_id.clone(),
        }
    }

    pub(super) fn user(context: &RequestContext, user_id: i32, request: &UserTaskRequest) -> Self {
        Self {
            user_id,
            activity_id: None,
            provider: if matches!(
                request.action,
                UserAction::StravaSync | UserAction::StravaBackfill { .. }
            ) {
                "strava"
            } else {
                "activity_processing"
            },
            action: json!(request.action),
            command_id: request.command_id,
            actor_id: context.actor_id,
            request_id: context.request_id.clone(),
        }
    }

    pub(super) async fn finish(
        self,
        state: &AppStorage,
        result: Result<JobReference, AppError>,
    ) -> Result<AcceptedTask, AppError> {
        let job = result.as_ref().ok();
        let (event_type, level, message) = if job.is_some() {
            (
                "admin.task.accepted",
                "info",
                "Manual task accepted by Temporal. Open the job for its execution result.",
            )
        } else {
            (
                "admin.task.submission_failed",
                "error",
                "Manual task acceptance was not confirmed. Retry the same request.",
            )
        };
        let recorded = record_event(
            &state.db,
            NewIntegrationEvent {
                user_id: Some(self.user_id),
                provider: self.provider.into(),
                event_type: event_type.into(),
                level: level.into(),
                message: message.into(),
                connection_id: None,
                payload: Some(json!({
                    "action": self.action, "command_id": self.command_id, "actor_user_id": self.actor_id,
                    "request_id": self.request_id, "activity_id": self.activity_id,
                    "http_status": result.as_ref().err().map(|error| error.status.as_u16()),
                    "rejection_reason": result.as_ref().err()
                        .filter(|error| error.status.is_client_error()).map(|error| &error.message),
                    "temporal_workflow_id": job.map(|job| &job.job_id),
                    "temporal_run_id": job.map(|job| &job.run_id),
                    "temporal_namespace": job.map(|job| &job.namespace),
                })),
            },
        )
        .await;
        // A failed history write must not turn an unacknowledged start into success.
        let job = result?;
        recorded?;
        Ok(AcceptedTask {
            status: "accepted".into(),
            job,
            user_id: self.user_id,
            activity_id: self.activity_id,
        })
    }
}
