use super::options::run;
use super::{
    logging::{self, ChildLog},
    options,
    sync::StartedSync,
    SyncActivities,
};
use crate::activities::ScanConnections;
use bike_core::strava::{
    events::StravaEvent,
    native::{ScanPage, ScanRequest},
};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{ChildWorkflowOptions, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ScheduledSync;

struct CustomerSync {
    child: StartedSync,
    log: ChildLog,
}
#[derive(Default)]
struct CustomerChildren {
    started: Vec<CustomerSync>,
    failed: u32,
}
#[workflow_methods]
impl ScheduledSync {
    #[run]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        request: Option<ScanRequest>,
    ) -> WorkflowResult<()> {
        let request = Self::get_request(request, ctx.run_id())?;
        logging::scan_started(ctx, &request);
        let result = Self::scan(ctx, request.clone()).await;
        let result = options::workflow_result(ctx, result);
        logging::scan_finished(ctx, &request, &result);
        result
    }

    async fn scan(ctx: &mut WorkflowContext<Self>, mut request: ScanRequest) -> WorkflowResult<()> {
        let page = Self::get_page(ctx, &request).await?;
        let children = Self::get_children(ctx, page.events).await?;
        request.failed_customers += children.failed;
        request.failed_customers += Self::complete_children(ctx, children.started).await;
        Self::finish_page(ctx, request, page.next_connection_id)
    }

    fn get_request(request: Option<ScanRequest>, run_id: &str) -> WorkflowResult<ScanRequest> {
        match request {
            Some(request) => Ok(request),
            None => Ok(ScanRequest {
                correlation_id: uuid::Uuid::parse_str(run_id)
                    .map_err(|error| options::failure(error.to_string()))?,
                after_connection_id: 0,
                failed_customers: 0,
            }),
        }
    }

    async fn get_page(
        ctx: &mut WorkflowContext<Self>,
        request: &ScanRequest,
    ) -> WorkflowResult<ScanPage> {
        run(ctx, ScanConnections, request.clone()).await
    }

    async fn get_children(
        ctx: &mut WorkflowContext<Self>,
        events: Vec<StravaEvent>,
    ) -> WorkflowResult<CustomerChildren> {
        let mut children = CustomerChildren::default();
        for event in events {
            let options = ChildWorkflowOptions::builder()
                .workflow_id(event.workflow_id())
                .search_attributes(crate::client::search_attributes(&event))
                .build();
            match options::start_child_workflow(ctx, SyncActivities::run, event, options).await {
                Ok((child, log)) => children.started.push(CustomerSync { child, log }),
                Err(error) if ctx.cancellation_token().is_cancelled() => return Err(error),
                Err(error) => {
                    if !ctx.patched("strava-scheduled-start-failure-v1") {
                        return Err(error);
                    }
                    children.failed += 1;
                }
            }
        }
        Ok(children)
    }

    async fn complete_children(ctx: &WorkflowContext<Self>, children: Vec<CustomerSync>) -> u32 {
        let mut failures = 0;
        // Await every customer even when an earlier child fails.
        for CustomerSync { child, log } in children {
            let result = child
                .result()
                .await
                .map_err(temporalio_workflow::WorkflowTermination::from);
            log.finished(ctx, &result);
            if result.is_err() {
                failures += 1;
            }
        }
        failures
    }

    fn finish_page(
        ctx: &mut WorkflowContext<Self>,
        request: ScanRequest,
        next_connection_id: Option<i32>,
    ) -> WorkflowResult<()> {
        if let Some(after_connection_id) = next_connection_id {
            Self::continue_scan(ctx, request, after_connection_id)
        } else if request.failed_customers > 0 {
            Err(options::failure(format!(
                "{} customer syncs failed; inspect child histories",
                request.failed_customers
            )))
        } else {
            Ok(())
        }
    }

    fn continue_scan(
        ctx: &mut WorkflowContext<Self>,
        request: ScanRequest,
        after_connection_id: i32,
    ) -> WorkflowResult<()> {
        let request = Some(ScanRequest {
            after_connection_id,
            ..request
        });
        options::validate_input(ctx, &request)?;
        ctx.continue_as_new(request, Default::default())
            .map(|never| match never {})
    }
}
