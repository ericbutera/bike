use super::super::{batches, options};
use crate::activities::ImportActivities;
use bike_core::bike_tasks::{
    ActivityImport, ActivityImportResult, BatchInput, ManifestScope, ReprocessScope,
    ReprocessSelection,
};
use temporalio_sdk::{ApplicationFailure, WorkflowContext, WorkflowResult};

pub(crate) async fn activity_import<W>(
    ctx: &WorkflowContext<W>,
    command: ActivityImport,
) -> WorkflowResult<ActivityImportResult> {
    command.validate().map_err(ApplicationFailure::new)?;
    let options = options::activity(
        command.source.priority,
        format!("user-{}", command.source.user_id),
    );
    options::run(ctx, ImportActivities::activity_import, command, options).await
}

pub(crate) async fn prepare_reprocessing<W>(
    ctx: &WorkflowContext<W>,
    batch: &mut BatchInput,
    scope: ReprocessScope,
) -> WorkflowResult<()> {
    let selection = ReprocessSelection {
        command: batch.command.clone(),
        scope,
    };
    let expected = ManifestScope::Reprocessing {
        user_id: batch.command.user_id,
        selection: scope,
    };
    batches::prepare(
        ctx,
        batch,
        ImportActivities::prepare_reprocess_imports,
        selection,
        expected,
    )
    .await
}
