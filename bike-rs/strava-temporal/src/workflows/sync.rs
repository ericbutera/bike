use super::options::run;
use super::{collection, logging, options};
use crate::activities::{CollectionWindow, CompleteSync};
use bike_core::strava::{events::StravaEvent, native::CollectionResult};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct SyncActivities;
pub(super) type StartedSync = temporalio_sdk::StartedChildWorkflow<sync_activities::Run>;
#[workflow_methods]
impl SyncActivities {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<CollectionResult> {
        logging::started(ctx, &event);
        let result = Self::sync(ctx, event.clone()).await;
        let result = options::workflow_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| &value.event);
        logging::finished(ctx, log_event, &result);
        result
    }
    async fn sync(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<CollectionResult> {
        let input = run(ctx, CollectionWindow, event).await?;
        let result = match collection::upsert_pages(ctx, input).await? {
            collection::CollectionStep::Completed(result) => result,
            collection::CollectionStep::Continue(event) => {
                options::validate_input(ctx, &event)?;
                return ctx
                    .continue_as_new(event, Default::default())
                    .map(|never| match never {});
            }
        };
        run(ctx, CompleteSync, result).await
    }
}
