use super::{domain_error, domain_task};
use async_trait::async_trait;
use bike_core::{
    bike_tasks::{ActivityTask, FitnessChange, SegmentChange},
    workflow_error::WorkflowError,
};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

#[async_trait]
pub trait DerivedOperations: Send + Sync {
    async fn rebuild_activity_segments(&self, task: ActivityTask) -> Result<Vec<i32>, WorkflowError>;
    async fn rebuild_activity_achievements(&self, task: ActivityTask) -> Result<(), WorkflowError>;
    async fn rebuild_activity_training(&self, task: ActivityTask) -> Result<(), WorkflowError>;
    async fn rebuild_activity_climbs(&self, task: ActivityTask) -> Result<(), WorkflowError>;
    async fn rebuild_activity_card(&self, task: ActivityTask) -> Result<(), WorkflowError>;
    async fn rebuild_fitness_freshness(&self, task: FitnessChange) -> Result<(), WorkflowError>;
    async fn rebuild_segment_analytics(&self, task: SegmentChange) -> Result<(), WorkflowError>;
    async fn prepare_heatmap(&self, task: ActivityTask) -> Result<(), WorkflowError>;
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
    #[activity(name = "rebuild_activity_segments")]
    pub async fn rebuild_activity_segments(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<Vec<i32>, ActivityError> {
        domain_task(async move { self.operations.rebuild_activity_segments(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_activity_achievements")]
    pub async fn rebuild_activity_achievements(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_activity_achievements(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_activity_training")]
    pub async fn rebuild_activity_training(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_activity_training(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_activity_climbs")]
    pub async fn rebuild_activity_climbs(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_activity_climbs(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_activity_card")]
    pub async fn rebuild_activity_card(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_activity_card(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_fitness_freshness")]
    pub async fn rebuild_fitness_freshness(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: FitnessChange,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_fitness_freshness(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_segment_analytics")]
    pub async fn rebuild_segment_analytics(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: SegmentChange,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_segment_analytics(task).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "prepare_heatmap")]
    pub async fn prepare_heatmap(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.prepare_heatmap(task).await })
            .await
            .map_err(domain_error)
    }
}
