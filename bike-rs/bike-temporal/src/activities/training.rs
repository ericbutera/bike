use super::activity_error;
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{
        BatchCommand, BatchManifest, PageRequest, TrainingActivityInput, TrainingActivityResult, TrainingCompletion,
    },
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

#[async_trait]
pub trait TrainingOperations: Send + Sync {
    async fn prepare_training_backfill(
        &self,
        command: BatchCommand,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError>;
    async fn training_activity_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<TrainingActivityInput>, WorkflowError>;
    async fn rebuild_activity_training(
        &self,
        command: TrainingActivityInput,
        context: ActivityContext,
    ) -> Result<TrainingActivityResult, WorkflowError>;
    async fn complete_training_backfill(
        &self,
        completion: TrainingCompletion,
        context: ActivityContext,
    ) -> Result<(), WorkflowError>;
}
pub struct TrainingActivities {
    operations: Arc<dyn TrainingOperations>,
}
impl TrainingActivities {
    pub fn new(operations: Arc<dyn TrainingOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl TrainingActivities {
    #[activity(name = "prepare_training_backfill")]
    pub async fn prepare_training_backfill(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: BatchCommand,
    ) -> Result<BatchManifest, ActivityError> {
        self.operations
            .prepare_training_backfill(command, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "training_activity_page")]
    pub async fn training_activity_page(
        self: Arc<Self>,
        ctx: ActivityContext,
        page: PageRequest,
    ) -> Result<Vec<TrainingActivityInput>, ActivityError> {
        self.operations
            .training_activity_page(page, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "rebuild_activity_training")]
    pub async fn rebuild_activity_training(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: TrainingActivityInput,
    ) -> Result<TrainingActivityResult, ActivityError> {
        self.operations
            .rebuild_activity_training(command, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "complete_training_backfill")]
    pub async fn complete_training_backfill(
        self: Arc<Self>,
        ctx: ActivityContext,
        completion: TrainingCompletion,
    ) -> Result<(), ActivityError> {
        self.operations
            .complete_training_backfill(completion, ctx)
            .await
            .map_err(activity_error)
    }
}
