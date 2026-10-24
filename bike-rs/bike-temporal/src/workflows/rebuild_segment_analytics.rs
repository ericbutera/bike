use super::options;
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::SegmentAnalyticsTask;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RebuildSegmentAnalytics;

#[workflow_methods]
impl RebuildSegmentAnalytics {
    #[run(name = "rebuild_segment_analytics")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, task: SegmentAnalyticsTask) -> WorkflowResult<()> {
        let first_id = task.segment_ids().first().copied().unwrap_or_default();
        let options = options::activity(task.priority(), format!("segment-{first_id}"));
        options::run(ctx, DerivedActivities::rebuild_segment_analytics, task, options).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::{bike_tasks::Priority, test_support as fixtures};
    use workflow_test_support::WorkflowMock;

    #[test]
    fn legacy_single_segment_payload_preserves_its_activity_input() {
        let task = fixtures::segment_change();
        let input: SegmentAnalyticsTask = serde_json::from_value(serde_json::to_value(&task).unwrap()).unwrap();
        let mut mock: WorkflowMock = WorkflowMock::new::<RebuildSegmentAnalytics>(input);
        mock.activity("rebuild_segment_analytics", &task, &());
        mock.result::<()>();
    }

    #[test]
    fn one_batch_activity_receives_every_segment_and_failure_stops_the_workflow() {
        let task = SegmentAnalyticsTask::Batch {
            segment_ids: vec![18, 19],
            priority: Priority::Background,
        };
        let mut mock: WorkflowMock = WorkflowMock::new::<RebuildSegmentAnalytics>(task.clone());
        mock.activity("rebuild_segment_analytics", &task, &());
        mock.result::<()>();
        let mut mock: WorkflowMock = WorkflowMock::new::<RebuildSegmentAnalytics>(task);
        mock.fail_activity("rebuild_segment_analytics", "changed source; retry");
        mock.assert_failure_message("changed source; retry");
    }
}
