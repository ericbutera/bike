use super::options::run;
use super::{logging, options};
use crate::activities::{
    CompleteActivity, DeleteSource, FetchActivity, FetchStreams, NormalizeActivity, PersistActivity, PublishHeatmap,
    RebuildActivity, RebuildFitness, RebuildSegments,
};
use bike_core::strava::native::ActivityStatus;
use bike_core::strava::{
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
        let result = Self::upsert(ctx, event.clone()).await;
        let result = options::workflow_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| &value.event);
        logging::finished(ctx, log_event, &result);
        result
    }

    async fn sources(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<NormalizedActivity> {
        let activity = run(ctx, FetchActivity, event).await?;
        let streams = run(ctx, FetchStreams, activity).await?;
        run(ctx, NormalizeActivity, streams).await
    }

    async fn upsert(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
        let normalized = Self::sources(ctx, event).await?;
        let change = run(ctx, PersistActivity, normalized).await?;
        if change.status == ActivityStatus::Superseded {
            return Ok(change);
        }
        if change.removed_source.is_some() {
            return Self::publish_exclusion(ctx, change).await;
        }
        Self::publish_activity(ctx, change).await
    }

    async fn publish_exclusion(
        ctx: &mut WorkflowContext<Self>,
        change: ActivityChange,
    ) -> WorkflowResult<ActivityChange> {
        let change = run(ctx, DeleteSource, change).await?;
        let change = run(ctx, RebuildSegments, change).await?;
        run(ctx, RebuildFitness, change).await
    }

    async fn publish_activity(
        ctx: &mut WorkflowContext<Self>,
        change: ActivityChange,
    ) -> WorkflowResult<ActivityChange> {
        let change = run(ctx, RebuildActivity, change).await?;
        let change = run(ctx, RebuildSegments, change).await?;
        let change = run(ctx, RebuildFitness, change).await?;
        let change = run(ctx, PublishHeatmap, change).await?;
        run(ctx, CompleteActivity, change).await
    }
}
