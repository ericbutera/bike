use super::super::batches;
use crate::activities::ArchiveActivities;
use bike_core::bike_tasks::{ArchiveCommand, ArchiveInput, ManifestScope};
use temporalio_sdk::{WorkflowContext, WorkflowResult};

pub(crate) async fn prepare<W>(ctx: &WorkflowContext<W>, input: &mut ArchiveInput) -> WorkflowResult<()> {
    let command = ArchiveCommand {
        batch: input.batch.command.clone(),
        archive_job_id: input.archive_job_id,
        source: input.source.clone(),
    };
    let expected = ManifestScope::Archive {
        user_id: input.batch.command.user_id,
        archive_job_id: input.archive_job_id,
    };
    batches::prepare(
        ctx,
        &mut input.batch,
        ArchiveActivities::prepare_archive_manifest,
        command,
        expected,
    )
    .await
}
