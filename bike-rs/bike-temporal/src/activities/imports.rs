use super::activity_error;
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{
        ActivityImport, ActivityImportResult, BatchManifest, ImportExecution, PageRequest, ReprocessInput,
        ReprocessSelection,
    },
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};
mod execution;

/// Idempotent domain effects. Temporal owns delivery and execution status.
#[async_trait]
pub trait ImportOperations: Send + Sync {
    async fn activity_import(
        &self,
        execution: ImportExecution,
        context: ActivityContext,
    ) -> Result<ActivityImportResult, WorkflowError>;
}

#[async_trait]
pub trait ReprocessManifestOperations: Send + Sync {
    async fn prepare_reprocess_imports(
        &self,
        selection: ReprocessSelection,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError>;
    async fn reprocess_import_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<ReprocessInput>, WorkflowError>;
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
    #[activity(name = "activity_import")]
    pub async fn activity_import(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: ActivityImport,
    ) -> Result<ActivityImportResult, ActivityError> {
        command.validate().map_err(super::domain_error)?;
        let priority = command.refresh_priority();
        let execution = execution::execution(&ctx, command.clone()).map_err(super::domain_error)?;
        let result = self
            .operations
            .activity_import(execution, ctx)
            .await
            .map(|result| result.with_priority(priority))
            .map_err(super::domain_error)?;
        command.validate_result(&result).map_err(activity_error)?;
        Ok(result)
    }
}

pub struct ReprocessManifestActivities {
    operations: Arc<dyn ReprocessManifestOperations>,
}

impl ReprocessManifestActivities {
    pub fn new(operations: Arc<dyn ReprocessManifestOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl ReprocessManifestActivities {
    #[activity(name = "prepare_reprocess_imports")]
    pub async fn prepare_reprocess_imports(
        self: Arc<Self>,
        ctx: ActivityContext,
        selection: ReprocessSelection,
    ) -> Result<BatchManifest, ActivityError> {
        self.operations
            .prepare_reprocess_imports(selection, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "reprocess_import_page")]
    pub async fn reprocess_import_page(
        self: Arc<Self>,
        ctx: ActivityContext,
        page: PageRequest,
    ) -> Result<Vec<ReprocessInput>, ActivityError> {
        self.operations
            .reprocess_import_page(page, ctx)
            .await
            .map_err(activity_error)
    }
}
