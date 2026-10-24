use super::{domain_error, domain_task};
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{
        ActivityImport, ActivityImportResult, ActivityProcessingExecution, ActivityProcessingResult,
        ActivityProcessingSource, ImportExecution,
    },
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};
mod execution;

#[async_trait]
pub trait ImportOperations: Send + Sync {
    async fn activity_import(
        &self,
        execution: ImportExecution,
        context: ActivityContext,
    ) -> Result<ActivityImportResult, WorkflowError>;

    async fn process_activity(
        &self,
        execution: ActivityProcessingExecution,
        context: ActivityContext,
    ) -> Result<ActivityProcessingResult, WorkflowError> {
        let ActivityProcessingSource::Import(command) = execution.source else {
            return Err(WorkflowError::bad_request("Provider processing is not configured"));
        };
        self.activity_import(
            ImportExecution {
                command: *command,
                workflow: execution.workflow,
            },
            context,
        )
        .await
        .map(ActivityProcessingResult::Import)
    }
}

pub struct ImportActivities {
    operations: Arc<dyn ImportOperations>,
}

impl ImportActivities {
    pub fn new(operations: Arc<dyn ImportOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl ImportActivities {
    #[activity(name = "process_activity")]
    pub async fn process_activity(
        self: Arc<Self>,
        ctx: ActivityContext,
        source: ActivityProcessingSource,
    ) -> Result<ActivityProcessingResult, ActivityError> {
        let command = source.clone();
        let execution = ActivityProcessingExecution {
            source,
            workflow: execution::workflow(&ctx).map_err(domain_error)?,
        };
        let result = domain_task(async move { self.operations.process_activity(execution, ctx).await })
            .await
            .map_err(domain_error)?;
        command.validate_result(&result).map_err(domain_error)?;
        Ok(result)
    }

    #[activity(name = "activity_import")]
    pub async fn activity_import(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: ActivityImport,
    ) -> Result<ActivityImportResult, ActivityError> {
        command.validate().map_err(domain_error)?;
        let execution = execution::execution(&ctx, command.clone()).map_err(domain_error)?;
        let result = domain_task(async move { self.operations.activity_import(execution, ctx).await })
            .await
            .map_err(domain_error)?;
        command.validate_result(&result).map_err(domain_error)?;
        Ok(result)
    }
}
