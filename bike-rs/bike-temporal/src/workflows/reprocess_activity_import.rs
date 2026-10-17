use super::{
    options::CommitBoundary,
    steps::{derived, imports},
};
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
        let boundary = CommitBoundary::begin(ctx)?;
        let imported = imports::activity_import(&boundary, input.into_command()).await?;
        derived::refresh_import_outputs(&boundary, &imported).await?;
        Ok(imported)
    }
}
