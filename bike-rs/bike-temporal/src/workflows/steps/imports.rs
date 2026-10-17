use super::super::options;
use crate::activities::ImportActivities;
use bike_core::bike_tasks::{ActivityImport, ActivityImportResult};
use temporalio_sdk::{ApplicationFailure, WorkflowContext, WorkflowResult};

pub(crate) async fn activity_import<W>(
    ctx: &WorkflowContext<W>,
    command: ActivityImport,
) -> WorkflowResult<ActivityImportResult> {
    command.validate().map_err(ApplicationFailure::new)?;
    let options = options::activity(command.source.priority, format!("user-{}", command.source.user_id));
    options::run(ctx, ImportActivities::activity_import, command, options).await
}
