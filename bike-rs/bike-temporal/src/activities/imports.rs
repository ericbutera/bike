use super::activity_error;
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{
        ActivityImport, ActivityImportResult, BatchManifest, PageRequest, ReprocessInput,
        ReprocessSelection,
    },
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

/// Source effects and stable manifest reads only. Committed command outcomes
/// must survive lost completion; preparation streams through shared storage.
#[async_trait]
pub trait ImportOperations: Send + Sync {
    async fn activity_import(
        &self,
        command: ActivityImport,
        context: ActivityContext,
    ) -> Result<ActivityImportResult, WorkflowError>;
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
        command.validate().map_err(activity_error)?;
        let priority = command.refresh_priority();
        self.operations
            .activity_import(command, ctx)
            .await
            .map(|result| result.with_priority(priority))
            .map_err(activity_error)
    }
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
