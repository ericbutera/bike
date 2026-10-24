use super::options::run;
use super::{logging, options};
use crate::activities::{
    CompleteActivity, DeleteSource, FetchActivity, FetchStreams, NormalizeActivity, PersistActivity, PublishHeatmap,
    RebuildActivity, RebuildFitness, RebuildSegments,
};
use integrations::strava::native::ActivityStatus;
use integrations::strava::{
    events::StravaEvent,
    native::{ActivityChange, NormalizedActivity},
};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct UpsertActivity;

#[workflow_methods]
impl UpsertActivity {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
        logging::started(ctx, &event);
        let result = if ctx.patched("strava-source-reconciliation-v1") {
            super::source::reconcile(ctx, event.clone()).await
        } else {
            upsert(ctx, event.clone()).await
        };
        let result = options::command_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| &value.event);
        logging::finished(ctx, log_event, &result);
        result
    }
}

async fn sources<W>(ctx: &mut WorkflowContext<W>, event: StravaEvent) -> WorkflowResult<NormalizedActivity> {
    let activity = run(ctx, FetchActivity, event).await?;
    let streams = run(ctx, FetchStreams, activity).await?;
    run(ctx, NormalizeActivity, streams).await
}

pub(super) async fn upsert<W>(ctx: &mut WorkflowContext<W>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
    if ctx.patched("strava-shared-activity-pipeline-v1") {
        return shared_activity(ctx, event).await;
    }
    let normalized = sources(ctx, event).await?;
    let boundary = options::CommitBoundary::begin(ctx)?;
    let change = boundary.run(PersistActivity, normalized).await?;
    if matches!(
        change.status,
        ActivityStatus::Superseded | ActivityStatus::SourceChanged
    ) {
        return Ok(change);
    }
    if change.removed_source.is_some() {
        return publish_exclusion(&boundary, change).await;
    }
    publish_activity(&boundary, change).await
}

async fn publish_exclusion<W>(
    boundary: &options::CommitBoundary<'_, W>,
    change: ActivityChange,
) -> WorkflowResult<ActivityChange> {
    let change = boundary.run(DeleteSource, change).await?;
    let change = boundary.run(RebuildSegments, change).await?;
    boundary.run(RebuildFitness, change).await
}

async fn publish_activity<W>(
    boundary: &options::CommitBoundary<'_, W>,
    change: ActivityChange,
) -> WorkflowResult<ActivityChange> {
    let change = boundary.run(RebuildActivity, change).await?;
    let change = boundary.run(RebuildSegments, change).await?;
    let change = boundary.run(RebuildFitness, change).await?;
    let change = boundary.run(PublishHeatmap, change).await?;
    boundary.run(CompleteActivity, change).await
}

async fn shared_activity<W>(ctx: &mut WorkflowContext<W>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
    use bike_core::bike_tasks::{ActivityProcessingResult, ActivityProcessingSource};
    use temporalio_sdk::{ChildWorkflowOptions, WorkflowCancellationToken};
    let activity = run(ctx, FetchActivity, event).await?;
    let streams = run(ctx, FetchStreams, activity).await?;
    let source = run(ctx, crate::activities::StravaActivities::retain_activity, streams).await?;
    options::CommitBoundary::begin(ctx)?;
    let options = ChildWorkflowOptions::builder()
        .workflow_id(format!("bike-source-{}", source.event.strava_event_id))
        .task_queue(bike_temporal::TASK_QUEUE.to_owned())
        .id_reuse_policy(temporalio_sdk::WorkflowIdReusePolicy::AllowDuplicate)
        .search_attributes(crate::client::search_attributes(&source.event))
        .cancellation_token(WorkflowCancellationToken::new())
        .build();
    let result = ctx
        .start_child_workflow(
            bike_temporal::workflows::ProcessActivity::run,
            ActivityProcessingSource::Strava(Box::new(source)),
            options,
        )
        .await?
        .result()
        .await?;
    match result {
        ActivityProcessingResult::Strava(change) => Ok(*change),
        _ => Err(options::failure(
            "Shared activity processing returned a different source result",
        )),
    }
}
