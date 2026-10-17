use super::{options, steps::derived};
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::ActivityTask;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RegenerateActivitySegments;

#[workflow_methods]
impl RegenerateActivitySegments {
    #[run(name = "regenerate_activity_segments")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, task: ActivityTask) -> WorkflowResult<()> {
        derived::activity_segments(ctx, task.clone()).await?;
        let options = options::activity(task.priority, format!("user-{}", task.user_id));
        options::run(ctx, DerivedActivities::rebuild_activity_achievements, task, options).await
    }
}
