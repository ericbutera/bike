use super::{
    debounce::{self, RebuildInput, Signals},
    options,
};
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::SegmentChange;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{SyncWorkflowContext, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RebuildSegmentAnalytics {
    changes: Signals<SegmentChange>,
}

#[workflow_methods]
impl RebuildSegmentAnalytics {
    #[run(name = "rebuild_segment_analytics")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: RebuildInput<SegmentChange>) -> WorkflowResult<()> {
        while let Some(batch) = debounce::next(ctx, &mut input, |s| &s.changes, |s| &mut s.changes).await? {
            let request = batch.request.clone();
            let options = options::activity(request.priority, format!("segment-{}", request.segment_id));
            let publication = options::run(ctx, DerivedActivities::rebuild_segment_analytics, request, options).await?;
            input.complete(batch, publication);
            if debounce::should_finish(ctx, &input, |s| &s.changes) {
                break;
            }
        }
        Ok(())
    }

    #[signal]
    pub fn drain(&mut self, _ctx: &mut SyncWorkflowContext<Self>) {
        self.changes.drain();
    }

    #[signal]
    pub fn changed(&mut self, ctx: &mut SyncWorkflowContext<Self>, request: SegmentChange) {
        self.changes.push(ctx, request);
    }
}
