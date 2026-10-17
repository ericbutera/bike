//! Replay-safe workflow lifecycle logs and parent/child execution links.
use integrations::strava::{events::StravaEvent, native::ScanRequest};
use temporalio_sdk::{WorkflowContext, WorkflowResult};
use temporalio_workflow::WorkflowTermination;
use uuid::Uuid;

#[derive(Clone, Copy)]
enum WorkflowStatus {
    Started,
    Completed,
    Failed,
    Cancelled,
    Continued,
}

impl WorkflowStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Continued => "continued",
        }
    }
}

struct Execution {
    namespace: String,
    workflow_id: String,
    run_id: Option<String>,
    workflow_type: String,
    parent_workflow_id: Option<String>,
    parent_run_id: Option<String>,
}

impl Execution {
    fn current<W>(ctx: &WorkflowContext<W>) -> Self {
        let parent = ctx.info().parent();
        Self {
            namespace: ctx.namespace().into(),
            workflow_id: ctx.workflow_id().into(),
            run_id: Some(ctx.run_id().into()),
            workflow_type: ctx.info().workflow_type().into(),
            parent_workflow_id: parent.as_ref().map(|parent| parent.workflow_id().into()),
            parent_run_id: parent.as_ref().map(|parent| parent.run_id().into()),
        }
    }

    fn child<W>(ctx: &WorkflowContext<W>, workflow_id: String, workflow_type: &str) -> Self {
        Self {
            namespace: ctx.namespace().into(),
            workflow_id,
            run_id: None,
            workflow_type: workflow_type.into(),
            parent_workflow_id: Some(ctx.workflow_id().into()),
            parent_run_id: Some(ctx.run_id().into()),
        }
    }

    fn span(&self, event: Option<&StravaEvent>, correlation_id: Uuid) -> tracing::Span {
        tracing::info_span!("strava.workflow",
            correlation_id = %correlation_id,
            strava_event_id = event.map(|event| event.strava_event_id.to_string()).as_deref(),
            parent_event_id = event.and_then(|event| event.parent_event_id).map(|id| id.to_string()).as_deref(),
            bike_customer_id = event.map(|event| event.bike_customer_id.to_string()).as_deref(),
            bike_user_id = event.map(|event| event.bike_user_id),
            bike_activity_id = event.and_then(|event| event.bike_activity_id),
            strava_athlete_id = event.map(|event| event.strava_athlete_id),
            strava_activity_id = event.and_then(|event| event.strava_activity_id),
            request_id = event.and_then(|event| event.request_id.as_deref()),
            trace_id = event.and_then(StravaEvent::trace_id).as_deref(),
            temporal_namespace = self.namespace,
            temporal_workflow_id = self.workflow_id,
            temporal_run_id = self.run_id.as_deref(),
            workflow_type = self.workflow_type,
            parent_temporal_workflow_id = self.parent_workflow_id.as_deref(),
            parent_temporal_run_id = self.parent_run_id.as_deref(),
            span_id = tracing::field::Empty)
    }
}

pub(super) fn started<W>(ctx: &WorkflowContext<W>, event: &StravaEvent) {
    record(
        ctx,
        Some(event),
        event.correlation_id,
        &Execution::current(ctx),
        WorkflowStatus::Started,
        None,
    );
}

pub(super) fn finished<W, T>(ctx: &WorkflowContext<W>, event: &StravaEvent, result: &WorkflowResult<T>) {
    record_result(ctx, Some(event), event.correlation_id, &Execution::current(ctx), result);
}

pub(super) fn scan_started<W>(ctx: &WorkflowContext<W>, request: &ScanRequest) {
    record(
        ctx,
        None,
        request.correlation_id,
        &Execution::current(ctx),
        WorkflowStatus::Started,
        None,
    );
}

pub(super) fn scan_finished<W, T>(ctx: &WorkflowContext<W>, request: &ScanRequest, result: &WorkflowResult<T>) {
    record_result(ctx, None, request.correlation_id, &Execution::current(ctx), result);
}

pub(super) struct ChildLog {
    event: StravaEvent,
    execution: Execution,
}

impl ChildLog {
    pub fn new<W>(ctx: &WorkflowContext<W>, event: StravaEvent, workflow_id: String, workflow_type: &str) -> Self {
        Self {
            event,
            execution: Execution::child(ctx, workflow_id, workflow_type),
        }
    }

    pub fn started<W>(&mut self, ctx: &WorkflowContext<W>, run_id: &str) {
        self.execution.run_id = Some(run_id.into());
        record(
            ctx,
            Some(&self.event),
            self.event.correlation_id,
            &self.execution,
            WorkflowStatus::Started,
            None,
        );
    }

    pub fn finished<W, T>(&self, ctx: &WorkflowContext<W>, result: &WorkflowResult<T>) {
        record_result(
            ctx,
            Some(&self.event),
            self.event.correlation_id,
            &self.execution,
            result,
        );
    }
}

fn record_result<W, T>(
    ctx: &WorkflowContext<W>,
    event: Option<&StravaEvent>,
    correlation_id: Uuid,
    execution: &Execution,
    result: &WorkflowResult<T>,
) {
    let (status, error) = match result {
        Ok(_) => (WorkflowStatus::Completed, None),
        Err(WorkflowTermination::Cancelled { .. }) => (WorkflowStatus::Cancelled, None),
        Err(WorkflowTermination::ContinueAsNew(_)) => (WorkflowStatus::Continued, None),
        Err(WorkflowTermination::Evicted) => return,
        Err(error) => (WorkflowStatus::Failed, Some(error.to_string())),
    };
    record(ctx, event, correlation_id, execution, status, error.as_deref());
}

fn record<W>(
    ctx: &WorkflowContext<W>,
    event: Option<&StravaEvent>,
    correlation_id: Uuid,
    execution: &Execution,
    status: WorkflowStatus,
    error: Option<&str>,
) {
    if ctx.is_replaying() {
        return;
    }
    emit(event, correlation_id, execution, status, error);
}

fn emit(
    event: Option<&StravaEvent>,
    correlation_id: Uuid,
    execution: &Execution,
    status: WorkflowStatus,
    error: Option<&str>,
) {
    let span = execution.span(event, correlation_id);
    if let Some(event) = event {
        bike_core::observability::set_span_parent_from_carrier(&span, event.trace_context.as_ref());
        bike_core::observability::record_span_trace_context(&span);
    }
    match status {
        WorkflowStatus::Failed => {
            tracing::error!(parent: &span, status = status.as_str(), error, "Strava workflow")
        }
        _ => tracing::info!(parent: &span, status = status.as_str(), "Strava workflow"),
    }
}
