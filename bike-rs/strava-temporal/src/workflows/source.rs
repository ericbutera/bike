use super::{activity_delete, activity_upsert, logging, options};
use integrations::strava::{
    events::{SourceVersion, StravaEvent, StravaOperation},
    native::{ActivityChange, ActivityStatus},
};
use std::time::Duration;
use temporalio_common::error::{ChildWorkflowStartError, StartChildWorkflowExecutionFailedCause};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{
    ChildWorkflowCancellationType, ChildWorkflowOptions, ParentClosePolicy, WorkflowContext, WorkflowIdReusePolicy,
    WorkflowResult, WorkflowTermination,
};
use temporalio_workflow::common::WorkflowDefinition;

#[workflow]
#[derive(Default)]
pub struct ReconcileActivity;

#[workflow_methods]
impl ReconcileActivity {
    #[run]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut event: StravaEvent) -> WorkflowResult<ActivityChange> {
        logging::started(ctx, &event);
        // The fetch/capture Activity replaces this marker with the row version
        // observed before I/O. Old histories omit it and retain their old path.
        event.source_version = Some(SourceVersion::default());
        let result = Self::apply(ctx, event.clone()).await;
        logging::finished(ctx, &event, &result);
        result
    }

    async fn apply(ctx: &mut WorkflowContext<Self>, event: StravaEvent) -> WorkflowResult<ActivityChange> {
        loop {
            let change = match event.operation {
                StravaOperation::Delete => activity_delete::delete(ctx, event.clone()).await?,
                _ => activity_upsert::upsert(ctx, event.clone()).await?,
            };
            if change.status != ActivityStatus::SourceChanged {
                return Ok(change);
            }
            // A timed-out older Activity can outlive its child run. Its stale
            // snapshot cannot commit; refetch against the current domain row.
            if ctx.continue_as_new_suggested() {
                return ctx
                    .continue_as_new(event, Default::default())
                    .map(|never| match never {});
            }
        }
    }
}

pub(super) async fn reconcile<W>(ctx: &mut WorkflowContext<W>, event: StravaEvent) -> WorkflowResult<ActivityChange>
where
    W: temporalio_sdk::workflows::WorkflowImplementation,
    W::Run: WorkflowDefinition<Input = StravaEvent>,
{
    // Record the parent decision before any child/timer commands, including a
    // canceled timer. Completion must not introduce a new patch marker.
    ctx.patched("strava-commit-boundary-v1");
    options::workflow_result(ctx, Ok(()))?;
    options::validate_input(ctx, &event)?;
    let workflow_id = event
        .source_workflow_id()
        .map_err(|error| options::failure(error.message))?;
    let mut delay = Duration::from_secs(1);
    for _ in 0..128 {
        let child_options = ChildWorkflowOptions::builder()
            .workflow_id(workflow_id.clone())
            .id_reuse_policy(WorkflowIdReusePolicy::AllowDuplicate)
            .cancel_type(ChildWorkflowCancellationType::WaitCancellationCompleted)
            .parent_close_policy(ParentClosePolicy::RequestCancel)
            .search_attributes(crate::client::search_attributes(&event))
            .build();
        let mut log = logging::ChildLog::new(ctx, event.clone(), workflow_id.clone(), ReconcileActivity::run.name());
        match ctx
            .start_child_workflow(ReconcileActivity::run, event.clone(), child_options)
            .await
        {
            Ok(child) => {
                log.started(ctx, &child.run_id);
                let result = child.result().await.map_err(WorkflowTermination::from);
                log.finished(ctx, &result);
                return result;
            }
            Err(ChildWorkflowStartError::StartFailed {
                cause: StartChildWorkflowExecutionFailedCause::WorkflowAlreadyExists,
                ..
            }) => {
                // Each distinct event still gets its own fetch/apply. Attaching
                // it to an existing run would silently drop changed work.
                ctx.timer(delay).await;
                options::workflow_result(ctx, Ok(()))?;
                delay = (delay * 2).min(Duration::from_secs(30));
                if ctx.continue_as_new_suggested() {
                    break;
                }
            }
            Err(error) => {
                let result: WorkflowResult<ActivityChange> = Err(WorkflowTermination::from(error));
                log.finished(ctx, &result);
                return result;
            }
        }
    }
    ctx.continue_as_new(event, Default::default())
        .map(|never| match never {})
}
