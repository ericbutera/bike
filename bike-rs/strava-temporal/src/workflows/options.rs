use super::logging::ChildLog;
use bike_core::strava::events::StravaEvent;
use std::time::Duration;
use temporalio_common::data_converters::TemporalDeserializable;
use temporalio_common::{error::ActivityExecutionError, RetryPolicy};
use temporalio_sdk::{
    ActivityCancellationType, ActivityOptions, ChildWorkflowOptions, WorkflowContext,
    WorkflowResult,
};
use temporalio_workflow::common::WorkflowDefinition;
use temporalio_workflow::{common::ActivityDefinition, WorkflowTermination};

pub(super) async fn start_child_workflow<W, D: WorkflowDefinition<Input = StravaEvent> + 'static>(
    ctx: &mut WorkflowContext<W>,
    definition: D,
    event: StravaEvent,
    mut options: ChildWorkflowOptions,
) -> WorkflowResult<(temporalio_sdk::StartedChildWorkflow<D>, ChildLog)>
where
    D::Output: TemporalDeserializable + 'static,
{
    validate_input(ctx, &event)?;
    let workflow_id = options
        .workflow_id
        .clone()
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| event.workflow_id());
    options.workflow_id = Some(workflow_id.clone());
    let mut log = ChildLog::new(ctx, event.clone(), workflow_id, definition.name());
    let result = ctx
        .start_child_workflow(definition, event, options)
        .await
        .map_err(WorkflowTermination::from);
    if result.is_err() {
        log.finished(ctx, &result);
    }
    let child = result?;
    log.started(ctx, &child.run_id);
    Ok((child, log))
}

pub(super) async fn execute_child_workflow<
    W,
    D: WorkflowDefinition<Input = StravaEvent> + 'static,
>(
    ctx: &mut WorkflowContext<W>,
    definition: D,
    event: StravaEvent,
    options: ChildWorkflowOptions,
) -> WorkflowResult<D::Output>
where
    D::Output: TemporalDeserializable + 'static,
{
    let (child, log) = start_child_workflow(ctx, definition, event, options).await?;
    let result = child.result().await.map_err(WorkflowTermination::from);
    log.finished(ctx, &result);
    result
}

pub(super) fn workflow_result<W, T>(
    ctx: &WorkflowContext<W>,
    result: WorkflowResult<T>,
) -> WorkflowResult<T> {
    if ctx.cancellation_token().is_cancelled() {
        Err(WorkflowTermination::cancelled())
    } else {
        result
    }
}

pub(super) async fn run<W, A: ActivityDefinition>(
    ctx: &mut WorkflowContext<W>,
    definition: A,
    input: A::Input,
) -> WorkflowResult<A::Output>
where
    A::Input: serde::Serialize,
{
    validate_input(ctx, &input)?;
    let name = definition.name();
    let summary = name
        .strip_prefix("strava_")
        .unwrap_or(name)
        .replace('_', " ");
    ctx.execute_activity(definition, input, activity(&summary))
        .await
        .map_err(termination)
}

pub(super) fn validate_input<W, T: serde::Serialize>(
    ctx: &WorkflowContext<W>,
    input: &T,
) -> WorkflowResult<()> {
    match bike_core::strava::payload::validate_temporal(input) {
        Err(error) if ctx.patched("strava-payload-budget-v1") => Err(failure(error.to_string())),
        _ => Ok(()),
    }
}

pub(super) fn activity(summary: &str) -> ActivityOptions {
    ActivityOptions::with_start_to_close_timeout(Duration::from_secs(120))
        .heartbeat_timeout(Duration::from_secs(30))
        .cancellation_type(ActivityCancellationType::WaitCancellationCompleted)
        .retry_policy(
            RetryPolicy::builder()
                .maximum_attempts(12)
                .maximum_interval(Duration::from_secs(60))
                .build(),
        )
        .summary(summary.to_owned())
        .build()
}

pub(super) fn termination(error: ActivityExecutionError) -> WorkflowTermination {
    if error.as_cancelled().is_some() {
        WorkflowTermination::cancelled()
    } else {
        error.into()
    }
}

pub(super) fn failure(message: impl Into<String>) -> WorkflowTermination {
    temporalio_common::error::ApplicationFailure::non_retryable(std::io::Error::other(
        message.into(),
    ))
    .into()
}

pub(super) fn upsert_child(event: &bike_core::strava::events::StravaEvent) -> ChildWorkflowOptions {
    ChildWorkflowOptions::builder()
        .workflow_id(format!("{}:upsert", event.workflow_id()))
        .search_attributes(crate::client::search_attributes(event))
        .build()
}
