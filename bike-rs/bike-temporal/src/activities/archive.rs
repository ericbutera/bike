use super::activity_error;
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{ArchiveCommand, BatchManifest, ImportSource, PageRequest},
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

/// Retain bytes and a streaming, immutable entry manifest through shared storage.
/// Heartbeat progress is advisory; repeated entries must recover committed refs.
#[async_trait]
pub trait ArchiveOperations: Send + Sync {
    async fn prepare_archive_manifest(
        &self,
        command: ArchiveCommand,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError>;
    async fn archive_import_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<ImportSource>, WorkflowError>;
}
pub struct ArchiveActivities {
    operations: Arc<dyn ArchiveOperations>,
}
impl ArchiveActivities {
    pub fn new(operations: Arc<dyn ArchiveOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl ArchiveActivities {
    #[activity(name = "prepare_archive_manifest")]
    pub async fn prepare_archive_manifest(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: ArchiveCommand,
    ) -> Result<BatchManifest, ActivityError> {
        self.operations
            .prepare_archive_manifest(command, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "archive_import_page")]
    pub async fn archive_import_page(
        self: Arc<Self>,
        ctx: ActivityContext,
        page: PageRequest,
    ) -> Result<Vec<ImportSource>, ActivityError> {
        self.operations
            .archive_import_page(page, ctx)
            .await
            .map_err(activity_error)
    }
}
