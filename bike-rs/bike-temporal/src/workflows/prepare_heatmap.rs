use super::{
    debounce::{self, RebuildInput, Signals},
    options,
};
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::HeatmapChange;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{SyncWorkflowContext, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct PrepareHeatmap {
    changes: Signals<HeatmapChange>,
}

#[workflow_methods]
impl PrepareHeatmap {
    #[run(name = "prepare_heatmap")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: RebuildInput<HeatmapChange>) -> WorkflowResult<()> {
        loop {
            let batch = debounce::next(ctx, &mut input, |s| &s.changes, |s| &mut s.changes).await?;
            let request = batch.request.clone();
            let options = options::activity(request.priority, format!("user-{}", request.user_id));
            let publication = options::run(ctx, DerivedActivities::prepare_heatmap, request, options).await?;
            input.complete(batch, publication);
        }
    }

    #[signal]
    pub fn changed(&mut self, ctx: &mut SyncWorkflowContext<Self>, request: HeatmapChange) {
        self.changes.push(ctx, request);
    }
}
