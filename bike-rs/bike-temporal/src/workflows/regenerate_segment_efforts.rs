use super::options;
use crate::activities::BatchActivities;
use bike_core::bike_tasks::SegmentChange;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RegenerateSegmentEfforts;

#[workflow_methods]
impl RegenerateSegmentEfforts {
    #[run(name = "regenerate_segment_efforts")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, task: SegmentChange) -> WorkflowResult<()> {
        let options = options::activity(task.priority, format!("segment-{}", task.segment_id));
        options::run(
            ctx,
            BatchActivities::regenerate_segment_efforts,
            task.segment_id,
            options,
        )
        .await
    }
}
