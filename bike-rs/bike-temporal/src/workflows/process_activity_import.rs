use super::steps;
use bike_core::bike_tasks::{
    ActivityImport, ActivityImportResult, ActivityProcessingResult, ActivityProcessingSource, ImportSource,
    ImportTarget,
};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{ApplicationFailure, WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ProcessActivity;

#[workflow_methods]
impl ProcessActivity {
    #[run(name = "process_activity")]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        source: ActivityProcessingSource,
    ) -> WorkflowResult<ActivityProcessingResult> {
        steps::process(ctx, source).await
    }
}

#[workflow]
#[derive(Default)]
pub struct ProcessActivityImport;

#[workflow_methods]
impl ProcessActivityImport {
    #[run(name = "process_activity_import")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, source: ImportSource) -> WorkflowResult<ActivityImportResult> {
        let result = steps::process(
            ctx,
            ActivityProcessingSource::Import(Box::new(ActivityImport {
                source,
                target: ImportTarget::NewActivity,
            })),
        )
        .await?;
        match result {
            ActivityProcessingResult::Import(imported) => Ok(imported),
            _ => Err(ApplicationFailure::non_retryable("Unexpected provider result for a retained import").into()),
        }
    }
}
