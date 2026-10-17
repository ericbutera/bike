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
        let result = Self::delete(ctx, event.clone()).await;
        let result = options::workflow_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| &value.event);
        logging::finished(ctx, log_event, &result);
        result
    }

    async fn delete(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
        let change = run(ctx, CaptureDeletion, event).await?;
        let change = run(ctx, DeleteBikeActivity, change).await?;
        if change.status == ActivityStatus::Superseded {
            return Ok(change);
        }
        let change = run(ctx, DeleteSource, change).await?;
        let change = run(ctx, RebuildSegments, change).await?;
        let change = run(ctx, RebuildFitness, change).await?;
        run(ctx, CompleteActivity, change).await
    }
}
