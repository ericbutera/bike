use super::{batches, steps::archive, ProcessActivityImport};
use crate::activities::ArchiveActivities;
use bike_core::bike_tasks::{ArchiveInput, BatchResult};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

#[workflow]
#[derive(Default)]
pub struct ActivityArchiveImport;

#[workflow_methods]
impl ActivityArchiveImport {
    #[run(name = "activity_archive_import")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut input: ArchiveInput) -> WorkflowResult<BatchResult> {
        archive::prepare(ctx, &mut input).await?;
        let step = batches::run_children(ctx, &mut input.batch, ArchiveActivities::archive_import_page, || {
            ProcessActivityImport::run
        })
        .await?;
        batches::finish_or_continue(ctx, input, step)
    }
}
