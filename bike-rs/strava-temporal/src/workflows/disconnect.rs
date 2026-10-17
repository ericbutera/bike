use super::options::run;
use super::{logging, options, DeleteActivity};
use crate::activities::{AcceptChild, BeginDisconnect, RemoveConnection, RevokeConnection};
use bike_core::strava::{
    events::{StravaEvent, StravaOperation},
    native::ChildEvent,
};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{ChildWorkflowOptions, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct DisconnectStrava;
#[workflow_methods]
impl DisconnectStrava {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<StravaEvent> {
        logging::started(ctx, &event);
        let result = Self::disconnect(ctx, event.clone()).await;
        let result = options::workflow_result(ctx, result);
        let log_event = result.as_ref().map_or(&event, |value| value);
        logging::finished(ctx, log_event, &result);
        result
    }
    async fn disconnect(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<StravaEvent> {
        let ids = run(ctx, BeginDisconnect, event.clone()).await?;
        let more = ids.len() == 50;
        for id in ids {
            let child = event.for_activity(id, StravaOperation::Delete);
            let child = run(
                ctx,
                AcceptChild,
                ChildEvent {
                    parent: event.clone(),
                    child,
                },
            )
            .await?;
            let options = ChildWorkflowOptions::builder()
                .workflow_id(child.workflow_id())
                .search_attributes(crate::client::search_attributes(&child))
                .build();
            options::execute_child_workflow(ctx, DeleteActivity::run, child, options).await?;
        }
        if more {
            options::validate_input(ctx, &event)?;
            return ctx
                .continue_as_new(event, Default::default())
                .map(|never| match never {});
        }
        let event = run(ctx, RevokeConnection, event).await?;
        run(ctx, RemoveConnection, event).await
    }
}
