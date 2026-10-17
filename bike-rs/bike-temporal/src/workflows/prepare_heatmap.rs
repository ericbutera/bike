use super::options;
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::ActivityTask;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct PrepareHeatmap;

#[workflow_methods]
impl PrepareHeatmap {
    #[run(name = "prepare_heatmap")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, task: ActivityTask) -> WorkflowResult<()> {
        let options = options::activity(task.priority, format!("user-{}", task.user_id));
        options::run(ctx, DerivedActivities::prepare_heatmap, task, options).await
    }
}
