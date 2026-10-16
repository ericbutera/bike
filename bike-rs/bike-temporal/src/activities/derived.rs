use super::activity_error;
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{FitnessChange, HeatmapChange, Publication, SegmentChange},
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

/// Each operation publishes under its committed generation/attempt fence.
#[async_trait]
pub trait DerivedOperations: Send + Sync {
    async fn rebuild_fitness_freshness(
        &self,
        change: FitnessChange,
        context: ActivityContext,
    ) -> Result<Publication, WorkflowError>;
    async fn rebuild_segment_analytics(
        &self,
        change: SegmentChange,
        context: ActivityContext,
    ) -> Result<Publication, WorkflowError>;
    async fn prepare_heatmap(
        &self,
        change: HeatmapChange,
        context: ActivityContext,
    ) -> Result<Publication, WorkflowError>;
}
pub struct DerivedActivities {
    operations: Arc<dyn DerivedOperations>,
}
impl DerivedActivities {
    pub fn new(operations: Arc<dyn DerivedOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl DerivedActivities {
    #[activity(name = "rebuild_fitness_freshness")]
    pub async fn rebuild_fitness_freshness(
        self: Arc<Self>,
        ctx: ActivityContext,
        change: FitnessChange,
    ) -> Result<Publication, ActivityError> {
        self.operations
            .rebuild_fitness_freshness(change, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "rebuild_segment_analytics")]
    pub async fn rebuild_segment_analytics(
        self: Arc<Self>,
        ctx: ActivityContext,
        change: SegmentChange,
    ) -> Result<Publication, ActivityError> {
        self.operations
            .rebuild_segment_analytics(change, ctx)
            .await
            .map_err(activity_error)
    }
    #[activity(name = "prepare_heatmap")]
    pub async fn prepare_heatmap(
        self: Arc<Self>,
        ctx: ActivityContext,
        change: HeatmapChange,
    ) -> Result<Publication, ActivityError> {
        self.operations
            .prepare_heatmap(change, ctx)
            .await
            .map_err(activity_error)
    }
}
