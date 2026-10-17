use super::{logging, options, UpsertActivity};
use bike_core::strava::{events::StravaEvent, native::ActivityChange};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct CreateActivity;

#[workflow_methods]
impl CreateActivity {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
        logging::started(ctx, &event);
        let options = options::upsert_child(&event);
        let result = options::execute_child_workflow(ctx, UpsertActivity::run, event.clone(), options).await;
        let result = options::command_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| &value.event);
        logging::finished(ctx, log_event, &result);
        result
    }
}
