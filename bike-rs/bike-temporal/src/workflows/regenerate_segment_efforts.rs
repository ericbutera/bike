use super::{batches, steps::segments, RegenerateActivitySegments};
use crate::activities::SegmentActivities;
use bike_core::bike_tasks::{BatchResult, SegmentEffortsInput};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RegenerateSegmentEfforts;

#[workflow_methods]
impl RegenerateSegmentEfforts {
    #[run(name = "regenerate_segment_efforts")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: SegmentEffortsInput) -> WorkflowResult<BatchResult> {
        segments::prepare_segment(ctx, &mut input).await?;
        let step = batches::run_children(ctx, &mut input.batch, SegmentActivities::segment_activity_page, || {
            RegenerateActivitySegments::run
        })
        .await?;
        batches::finish_or_continue(ctx, input, step)
    }
}
