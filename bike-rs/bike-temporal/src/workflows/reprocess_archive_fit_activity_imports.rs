use super::{batches, steps::imports, ReprocessActivityImport};
use crate::activities::ReprocessManifestActivities;
use bike_core::bike_tasks::{BatchInput, BatchResult, ReprocessScope};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ReprocessArchiveFitActivityImports;

#[workflow_methods]
impl ReprocessArchiveFitActivityImports {
    #[run(name = "reprocess_archive_fit_activity_imports")]
    pub async fn run(
        ctx: &mut WorkflowContext<Self>,
        mut input: BatchInput,
    ) -> WorkflowResult<BatchResult> {
        imports::prepare_reprocessing(ctx, &mut input, ReprocessScope::ArchiveFits).await?;
        let step = batches::run_children(
            ctx,
            &mut input,
            ReprocessManifestActivities::reprocess_import_page,
            || ReprocessActivityImport::run,
        )
        .await?;
        batches::finish_or_continue(ctx, input, step)
    }
}
