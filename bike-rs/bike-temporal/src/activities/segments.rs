use super::activity_error;
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{
        ActivitySegmentsInput, ActivitySegmentsResult, BatchCommand, BatchManifest, PageRequest,
        SegmentEffortsSelection,
    },
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

#[async_trait]
pub trait SegmentOperations: Send + Sync {
    async fn match_activity_segments(
        &self,
        command: ActivitySegmentsInput,
        context: ActivityContext,
    ) -> Result<ActivitySegmentsResult, WorkflowError>;
    async fn prepare_user_segments(
        &self,
        command: BatchCommand,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError>;
    async fn prepare_segment_efforts(
        &self,
        selection: SegmentEffortsSelection,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError>;
    async fn segment_activity_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<ActivitySegmentsInput>, WorkflowError>;
}
pub struct SegmentActivities {
    operations: Arc<dyn SegmentOperations>,
}
impl SegmentActivities {
    pub fn new(operations: Arc<dyn SegmentOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl SegmentActivities {
    #[activity(name = "match_activity_segments")]
    pub async fn match_activity_segments(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: ActivitySegmentsInput,
    ) -> Result<ActivitySegmentsResult, ActivityError> {
        self.operations
            .match_activity_segments(command, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "prepare_user_segments")]
    pub async fn prepare_user_segments(
        self: Arc<Self>,
        ctx: ActivityContext,
        command: BatchCommand,
    ) -> Result<BatchManifest, ActivityError> {
        self.operations
            .prepare_user_segments(command, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "prepare_segment_efforts")]
    pub async fn prepare_segment_efforts(
        self: Arc<Self>,
        ctx: ActivityContext,
        selection: SegmentEffortsSelection,
    ) -> Result<BatchManifest, ActivityError> {
        self.operations
            .prepare_segment_efforts(selection, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "segment_activity_page")]
    pub async fn segment_activity_page(
        self: Arc<Self>,
        ctx: ActivityContext,
        page: PageRequest,
    ) -> Result<Vec<ActivitySegmentsInput>, ActivityError> {
        self.operations
            .segment_activity_page(page, ctx)
            .await
            .map_err(activity_error)
    }
}
