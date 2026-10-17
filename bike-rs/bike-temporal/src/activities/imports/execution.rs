use bike_core::{
    bike_tasks::{ActivityImport, ImportExecution, JobReference},
    workflow_error::WorkflowError,
};
use temporalio_sdk::activities::ActivityContext;

pub(super) fn execution(ctx: &ActivityContext, command: ActivityImport) -> Result<ImportExecution, WorkflowError> {
    let info = ctx.info();
    let required = |value: &Option<String>| {
        value
            .clone()
            .ok_or_else(|| WorkflowError::bad_request("Import requires a Temporal workflow execution"))
    };
    Ok(ImportExecution {
        command,
        workflow: JobReference {
            namespace: info.namespace.clone(),
            job_id: required(&info.workflow_id)?,
            run_id: required(&info.workflow_run_id)?,
            workflow_type: required(&info.workflow_type)?,
        },
    })
}
