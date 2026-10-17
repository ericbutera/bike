use super::{children, options, PrepareHeatmap};
use crate::activities::BatchActivities;
use bike_core::bike_tasks::Priority;
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ReconcileHeatmaps;

#[workflow_methods]
impl ReconcileHeatmaps {
    #[run(name = "reconcile_heatmaps")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, after_id: i32) -> WorkflowResult<()> {
        let tasks = options::run(
            ctx,
            BatchActivities::pending_heatmaps,
            after_id,
            options::activity(Priority::Background, "heatmaps".into()),
        )
        .await?;
        let full = tasks.len() == 16;
        let mut next = after_id;
        for task in tasks {
            next = task.activity_id;
            let user_id = task.user_id;
            children::run(
                ctx,
                PrepareHeatmap::run,
                task,
                Priority::Background,
                format!("user-{user_id}"),
            )
            .await?;
        }
        if full {
            return ctx
                .continue_as_new(next, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}
