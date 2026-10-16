use super::{
    debounce::{self, RebuildInput, Signals},
    options,
};
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::FitnessChange;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{SyncWorkflowContext, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RebuildFitnessFreshness {
    changes: Signals<FitnessChange>,
}

#[workflow_methods]
impl RebuildFitnessFreshness {
    #[run(name = "rebuild_fitness_freshness")]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        mut input: RebuildInput<FitnessChange>,
    ) -> WorkflowResult<()> {
        loop {
            let batch = debounce::next(ctx, &mut input, |s| &s.changes, |s| &mut s.changes).await?;
            let request = batch.request.clone();
            let options = options::activity(request.priority, format!("user-{}", request.user_id));
            let publication = options::run(
                ctx,
                DerivedActivities::rebuild_fitness_freshness,
                request,
                options,
            )
            .await?;
            input.complete(batch, publication);
        }
    }

    #[signal]
    pub fn changed(&mut self, ctx: &mut SyncWorkflowContext<Self>, request: FitnessChange) {
        self.changes.push(ctx, request);
    }
}
