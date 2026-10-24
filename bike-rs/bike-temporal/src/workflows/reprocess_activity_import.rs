use super::steps;
use bike_core::bike_tasks::{ActivityImportResult, ActivityProcessingResult, ActivityProcessingSource, ReprocessInput};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{ApplicationFailure, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ReprocessActivityImport;

#[workflow_methods]
impl ReprocessActivityImport {
    #[run(name = "reprocess_activity_import")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, input: ReprocessInput) -> WorkflowResult<ActivityImportResult> {
        let result = steps::process(ctx, ActivityProcessingSource::Import(Box::new(input.into_command()))).await?;
        match result {
            ActivityProcessingResult::Import(imported) => Ok(imported),
            _ => Err(ApplicationFailure::non_retryable("Unexpected provider result for a retained import").into()),
        }
    }
}
