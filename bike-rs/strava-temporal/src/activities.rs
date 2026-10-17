//! Each named Temporal activity calls one provider or Bike domain operation.
use bike_core::{platform::temporal as policy, workflow_error::WorkflowError};
use integrations::strava::{
    events::StravaEvent,
    logging::{AttemptStatus, ExchangeContext, TemporalIds},
    native::*,
};
use std::{future::Future, sync::Arc};
use temporalio_common::error::ApplicationFailure;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};
use tracing::Instrument;

pub use strava_activities::{
    AcceptChild, BeginDisconnect, CaptureDeletion, CollectionWindow, CompleteActivity, CompleteBackfill, CompleteSync,
    DeleteActivity, DeleteSource, FetchActivity, FetchStreams, ListActivityPage, NormalizeActivity, PersistActivity,
    PublishHeatmap, RebuildActivity, RebuildFitness, RebuildSegments, RemoveConnection, RevokeConnection,
    ScanConnections,
};

pub struct StravaActivities {
    operations: StravaOperations,
}
impl StravaActivities {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self {
            operations: StravaOperations::new(db),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_operations(operations: StravaOperations) -> Self {
        Self { operations }
    }
}

#[activities]
impl StravaActivities {
    #[activity(name = "strava_scan_connections")]
    pub async fn scan_connections(
        self: Arc<Self>,
        ctx: ActivityContext,
        request: ScanRequest,
    ) -> Result<ScanPage, ActivityError> {
        let info = ctx.info();
        let span = tracing::info_span!("strava.scan", correlation_id = %request.correlation_id,
            temporal_namespace = info.namespace,
            temporal_workflow_id = info.workflow_id, temporal_run_id = info.workflow_run_id,
            temporal_activity_id = info.activity_id, temporal_attempt = info.attempt,
            trace_id = tracing::field::Empty, span_id = tracing::field::Empty);
        bike_core::observability::record_span_trace_context(&span);
        self.operations
            .scan_connections(request)
            .instrument(span)
            .await
            .and_then(checked_output)
            .map_err(activity_error)
    }
    #[activity(name = "strava_resolve_collection_window")]
    pub async fn collection_window(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StravaEvent,
    ) -> Result<CollectionInput, ActivityError> {
        run(ctx, input.clone(), self.operations.collection_window(input)).await
    }
    #[activity(name = "strava_accept_child_event")]
    pub async fn accept_child(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ChildEvent,
    ) -> Result<StravaEvent, ActivityError> {
        run(ctx, input.parent, self.operations.accept_child(input.child)).await
    }
    #[activity(name = "strava_complete_sync")]
    pub async fn complete_sync(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: CollectionResult,
    ) -> Result<CollectionResult, ActivityError> {
        run(ctx, input.event.clone(), self.operations.complete_sync(input)).await
    }
    #[activity(name = "strava_complete_backfill")]
    pub async fn complete_backfill(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: CollectionResult,
    ) -> Result<CollectionResult, ActivityError> {
        run(ctx, input.event.clone(), self.operations.complete_backfill(input)).await
    }
    #[activity(name = "strava_begin_disconnect")]
    pub async fn begin_disconnect(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StravaEvent,
    ) -> Result<Vec<i64>, ActivityError> {
        run(ctx, input.clone(), self.operations.begin_disconnect(input)).await
    }
    #[activity(name = "strava_revoke_connection")]
    pub async fn revoke_connection(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StravaEvent,
    ) -> Result<StravaEvent, ActivityError> {
        run(ctx, input.clone(), self.operations.revoke_connection(input)).await
    }
    #[activity(name = "strava_remove_connection")]
    pub async fn remove_connection(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StravaEvent,
    ) -> Result<StravaEvent, ActivityError> {
        run(ctx, input.clone(), self.operations.remove_connection(input)).await
    }
    #[activity(name = "strava_fetch_activity")]
    pub async fn fetch_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StravaEvent,
    ) -> Result<ActivitySource, ActivityError> {
        run(ctx, input.clone(), self.operations.fetch_activity(input)).await
    }
    #[activity(name = "strava_fetch_streams")]
    pub async fn fetch_streams(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivitySource,
    ) -> Result<ActivitySources, ActivityError> {
        run(ctx, input.event.clone(), self.operations.fetch_streams(input)).await
    }
    #[activity(name = "strava_normalize_activity")]
    pub async fn normalize_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivitySources,
    ) -> Result<NormalizedActivity, ActivityError> {
        run(ctx, input.event.clone(), self.operations.normalize_activity(input)).await
    }
    #[activity(name = "strava_persist_activity")]
    pub async fn persist_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: NormalizedActivity,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.persist_activity(input)).await
    }
    #[activity(name = "strava_capture_deletion")]
    pub async fn capture_deletion(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: StravaEvent,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.clone(), self.operations.capture_deletion(input)).await
    }
    #[activity(name = "strava_delete_activity")]
    pub async fn delete_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.delete_activity(input)).await
    }
    #[activity(name = "strava_delete_source")]
    pub async fn delete_source(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.delete_source(input)).await
    }
    #[activity(name = "strava_rebuild_activity")]
    pub async fn rebuild_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.rebuild_activity(input)).await
    }
    #[activity(name = "strava_rebuild_fitness")]
    pub async fn rebuild_fitness(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.rebuild_fitness(input)).await
    }
    #[activity(name = "strava_rebuild_segments")]
    pub async fn rebuild_segments(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.rebuild_segments(input)).await
    }
    #[activity(name = "strava_publish_heatmap")]
    pub async fn publish_heatmap(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.publish_heatmap(input)).await
    }
    #[activity(name = "strava_complete_activity")]
    pub async fn complete_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: ActivityChange,
    ) -> Result<ActivityChange, ActivityError> {
        run(ctx, input.event.clone(), self.operations.complete_activity(input)).await
    }
    #[activity(name = "strava_list_activity_page")]
    pub async fn list_activity_page(
        self: Arc<Self>,
        ctx: ActivityContext,
        input: PageInput,
    ) -> Result<ActivityPage, ActivityError> {
        run(ctx, input.collection.event.clone(), self.operations.list_page(input)).await
    }
}

async fn run<T: serde::Serialize>(
    ctx: ActivityContext,
    event: StravaEvent,
    future: impl Future<Output = Result<T, WorkflowError>>,
) -> Result<T, ActivityError> {
    let info = ctx.info();
    validate_execution(&event, info.workflow_type.as_deref(), info.workflow_id.as_deref())?;
    let context = ExchangeContext {
        event,
        temporal: TemporalIds {
            temporal_namespace: info.namespace.clone(),
            temporal_workflow_id: info.workflow_id.clone().unwrap_or_default(),
            temporal_run_id: info.workflow_run_id.clone().unwrap_or_default(),
            temporal_activity_id: info.activity_id.clone(),
            temporal_attempt: info.attempt,
        },
    };
    let logging = context.clone();
    context
        .scope(&info.activity_type, async {
            logging.attempt(&info.activity_type, AttemptStatus::Started);
            tokio::pin!(future);
            let mut heartbeat = tokio::time::interval(policy::ACTIVITY_HEARTBEAT_INTERVAL);
            let result = loop {
                tokio::select! {
                    result = &mut future => break result.and_then(checked_output).map_err(activity_error),
                    () = ctx.cancelled() => break Err(ActivityError::cancelled()),
                    _ = heartbeat.tick() => ctx.record_heartbeat(info.attempt).await?,
                }
            };
            match &result {
                Ok(_) => logging.attempt(&info.activity_type, AttemptStatus::Completed),
                Err(ActivityError::Cancelled { .. }) => logging.attempt(&info.activity_type, AttemptStatus::Cancelled),
                Err(_) => logging.attempt(&info.activity_type, AttemptStatus::Failed),
            }
            result
        })
        .await
}

fn validate_execution(
    event: &StravaEvent,
    workflow_type: Option<&str>,
    workflow_id: Option<&str>,
) -> Result<(), ActivityError> {
    use crate::workflows::*;
    use integrations::strava::events::StravaOperation;
    use temporalio_workflow::common::WorkflowDefinition;
    if workflow_type == Some(ReconcileActivity::run.name()) {
        let source_operation = matches!(
            event.operation,
            StravaOperation::Create | StravaOperation::Update | StravaOperation::Upsert | StravaOperation::Delete
        );
        if !source_operation {
            return Err(activity_error(WorkflowError::bad_request(
                "Strava collection is not source reconciliation",
            )));
        }
        let expected_id = event.source_workflow_id().map_err(activity_error)?;
        return require_workflow_id(workflow_id == Some(expected_id.as_str()));
    }
    let expected_workflow = match event.operation {
        StravaOperation::Create | StravaOperation::Update | StravaOperation::Upsert => UpsertActivity::run.name(),
        StravaOperation::Delete => DeleteActivity::run.name(),
        StravaOperation::Sync => SyncActivities::run.name(),
        StravaOperation::Backfill => BackfillActivities::run.name(),
        StravaOperation::Disconnect => DisconnectStrava::run.name(),
    };
    if workflow_type != Some(expected_workflow) {
        return Err(ApplicationFailure::non_retryable(std::io::Error::other(
            "Strava event operation does not match its Temporal workflow",
        ))
        .into());
    }
    let expected_id = event.workflow_id();
    let legacy_child = format!("{expected_id}:upsert");
    require_workflow_id(workflow_id == Some(expected_id.as_str()) || workflow_id == Some(legacy_child.as_str()))
}

fn require_workflow_id(matches: bool) -> Result<(), ActivityError> {
    if !matches {
        return Err(ApplicationFailure::non_retryable(std::io::Error::other(
            "Strava event does not belong to this Temporal execution",
        ))
        .into());
    }
    Ok(())
}

pub(crate) fn checked_output<T: serde::Serialize>(output: T) -> Result<T, WorkflowError> {
    integrations::strava::payload::validate_temporal(&output)?;
    Ok(output)
}

fn activity_error(error: WorkflowError) -> ActivityError {
    let delay = error.retry_at.map(|at| {
        (at - chrono::Utc::now())
            .to_std()
            .unwrap_or(policy::RETRY_INITIAL_INTERVAL)
            .max(policy::RETRY_INITIAL_INTERVAL)
    });
    let retryable = error.retry_at.is_some() || error.status.as_u16() == 429;
    let terminal = error.status.is_client_error() && !retryable;
    ApplicationFailure::builder(error)
        .type_name("StravaOperationFailure".to_owned())
        .non_retryable(terminal)
        .maybe_next_retry_delay(delay)
        .build()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn source_execution_requires_the_exact_owner_generation_and_remote_object() {
        use temporalio_workflow::common::WorkflowDefinition;
        let fixture = crate::tests::fixture::Fixture::new(integrations::strava::events::StravaOperation::Upsert);
        let workflow_type = Some(crate::workflows::ReconcileActivity::run.name());
        let workflow_id = fixture.event.source_workflow_id().unwrap();
        // A stored mutation from old history can omit its snapshot and still
        // complete in the new source child; execution ownership remains exact.
        validate_execution(&fixture.event, workflow_type, Some(&workflow_id)).unwrap();
        let mut wrong_owner = fixture.event.clone();
        wrong_owner.bike_user_id += 1;
        assert!(validate_execution(&wrong_owner, workflow_type, Some(&workflow_id)).is_err());
        assert!(validate_execution(&fixture.event, workflow_type, Some(&format!("{workflow_id}-other"))).is_err());
        assert!(validate_execution(&fixture.event, Some("SyncActivities"), Some(&workflow_id)).is_err());
    }

    #[test]
    fn provider_rate_limits_retry_but_invalid_requests_are_terminal() {
        for (status, terminal) in [(400, true), (404, true), (429, false), (500, false)] {
            let mut error = WorkflowError::internal("fixture failure");
            error.status = status.try_into().unwrap();
            let ActivityError::Application(failure) = activity_error(error) else {
                panic!("Expected application failure")
            };
            assert_eq!(failure.is_non_retryable(), terminal);
            assert_eq!(failure.type_name(), Some("StravaOperationFailure"));
        }
    }

    #[test]
    fn provider_retry_time_is_retained_and_expired_delays_have_a_one_second_minimum() {
        for seconds in [120, -120] {
            let mut error = WorkflowError::bad_request("fixture provider pause");
            error.retry_at = Some(chrono::Utc::now() + chrono::Duration::seconds(seconds));
            let ActivityError::Application(failure) = activity_error(error) else {
                panic!("Expected application failure")
            };
            assert!(!failure.is_non_retryable());
            let delay = failure.next_retry_delay().unwrap();
            if seconds < 0 {
                assert_eq!(delay, policy::RETRY_INITIAL_INTERVAL);
            } else {
                assert!(delay > Duration::from_secs(100));
            }
        }
    }

    #[test]
    fn oversized_activity_output_is_a_non_retryable_failure() {
        let error = checked_output(vec![i32::MAX; 30_000]).unwrap_err();
        let ActivityError::Application(failure) = activity_error(error) else {
            panic!("Expected a terminal application failure")
        };
        assert!(failure.is_non_retryable());
    }
}
