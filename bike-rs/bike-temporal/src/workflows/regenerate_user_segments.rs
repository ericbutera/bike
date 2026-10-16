use super::{batches, steps::segments, RegenerateActivitySegments};
use crate::activities::SegmentActivities;
use bike_core::bike_tasks::{BatchInput, BatchResult};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RegenerateUserSegments;

#[workflow_methods]
impl RegenerateUserSegments {
    #[run(name = "regenerate_user_segments")]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        mut input: BatchInput,
    ) -> WorkflowResult<BatchResult> {
        segments::prepare_user(ctx, &mut input).await?;
        let step = batches::run_children(
            ctx,
            &mut input,
            SegmentActivities::segment_activity_page,
            || RegenerateActivitySegments::run,
        )
        .await?;
        batches::finish_or_continue(ctx, input, step)
    }
}
