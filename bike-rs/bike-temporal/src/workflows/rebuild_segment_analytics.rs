use super::options;
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::SegmentChange;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RebuildSegmentAnalytics;

#[workflow_methods]
impl RebuildSegmentAnalytics {
    #[run(name = "rebuild_segment_analytics")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, task: SegmentChange) -> WorkflowResult<()> {
        let options = options::activity(task.priority, format!("segment-{}", task.segment_id));
        options::run(ctx, DerivedActivities::rebuild_segment_analytics, task, options).await
    }
}
