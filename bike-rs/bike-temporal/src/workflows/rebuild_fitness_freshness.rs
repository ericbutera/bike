use super::options;
use crate::activities::DerivedActivities;
use bike_core::bike_tasks::FitnessChange;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct RebuildFitnessFreshness;

#[workflow_methods]
impl RebuildFitnessFreshness {
    #[run(name = "rebuild_fitness_freshness")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, task: FitnessChange) -> WorkflowResult<()> {
        let options = options::activity(task.priority, format!("user-{}", task.user_id));
        options::run(ctx, DerivedActivities::rebuild_fitness_freshness, task, options).await
    }
}
