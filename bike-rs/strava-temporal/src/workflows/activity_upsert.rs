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
