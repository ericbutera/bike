use super::options::run;
use super::{logging, options};
use crate::activities::{
    CaptureDeletion, CompleteActivity, DeleteActivity as DeleteBikeActivity, DeleteSource, RebuildFitness,
    RebuildSegments,
};
use bike_core::strava::native::ActivityStatus;
use bike_core::strava::{events::StravaEvent, native::ActivityChange};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct DeleteActivity;

#[workflow_methods]
impl DeleteActivity {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
        logging::started(ctx, &event);
        let result = if ctx.patched("strava-source-reconciliation-v1") {
            super::source::reconcile(ctx, event.clone()).await
        } else {
            delete(ctx, event.clone()).await
        };
        let result = options::workflow_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| &value.event);
        logging::finished(ctx, log_event, &result);
        result
    }
}

pub(super) async fn delete<W>(ctx: &mut WorkflowContext<W>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
    let change = run(ctx, CaptureDeletion, event).await?;
    let change = run(ctx, DeleteBikeActivity, change).await?;
    if matches!(
        change.status,
        ActivityStatus::Superseded | ActivityStatus::SourceChanged
    ) {
        return Ok(change);
    }
    let change = run(ctx, DeleteSource, change).await?;
    let change = run(ctx, RebuildSegments, change).await?;
    let change = run(ctx, RebuildFitness, change).await?;
    run(ctx, CompleteActivity, change).await
}
