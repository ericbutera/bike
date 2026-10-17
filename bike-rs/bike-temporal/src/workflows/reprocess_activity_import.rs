use super::steps::{derived, imports};
use bike_core::bike_tasks::{ActivityImportResult, ReprocessInput};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ReprocessActivityImport;

#[workflow_methods]
impl ReprocessActivityImport {
    #[run(name = "reprocess_activity_import")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, input: ReprocessInput) -> WorkflowResult<ActivityImportResult> {
        let imported = imports::activity_import(ctx, input.into_command()).await?;
        derived::segments(ctx, &imported).await?;
        derived::achievements(ctx, &imported).await?;
        derived::heatmap(ctx, &imported).await?;
        derived::training_and_fitness(ctx, &imported).await?;
        Ok(imported)
    }
}
