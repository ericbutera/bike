use super::{
    options::CommitBoundary,
    steps::{derived, imports},
};
use bike_core::bike_tasks::{ActivityImport, ActivityImportResult, ImportSource, ImportTarget};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ProcessActivityImport;

#[workflow_methods]
impl ProcessActivityImport {
    #[run(name = "process_activity_import")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, source: ImportSource) -> WorkflowResult<ActivityImportResult> {
        let boundary = CommitBoundary::begin(ctx)?;
        let imported = imports::activity_import(
            &boundary,
            ActivityImport {
                source,
                target: ImportTarget::NewActivity,
            },
        )
        .await?;
        derived::refresh_import_outputs(&boundary, &imported).await?;
        Ok(imported)
    }
}
