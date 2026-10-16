use super::{
    batches::{self, BatchStep},
    steps::training,
};
use crate::activities::TrainingActivities;
use bike_core::bike_tasks::{BatchInput, BatchResult};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct BackfillUserXcTraining;

#[workflow_methods]
impl BackfillUserXcTraining {
    #[run(name = "backfill_user_xc_training")]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        mut input: BatchInput,
    ) -> WorkflowResult<BatchResult> {
        training::prepare(ctx, &mut input).await?;
        let step = batches::run_pages(
            ctx,
            &mut input,
            TrainingActivities::training_activity_page,
            |activity| training::rebuild_activity(ctx, activity),
        )
        .await?;
        if let BatchStep::Complete(result) = &step {
            training::complete(ctx, &input, result).await?;
        }
        batches::finish_or_continue(ctx, input, step)
    }
}
