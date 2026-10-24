use super::{domain_error, domain_task};
use async_trait::async_trait;
use bike_core::{bike_tasks::*, workflow_error::WorkflowError};
use std::sync::Arc;
use temporalio_macros::activities;
use temporalio_sdk::activities::{ActivityContext, ActivityError};

#[async_trait]
pub trait BatchOperations: Send + Sync {
    async fn activity_page(&self, input: ActivityPage) -> Result<Vec<ActivityTask>, WorkflowError>;
    async fn reprocess_input(&self, input: ActivityTask) -> Result<ReprocessInput, WorkflowError>;
    async fn regenerate_segment_efforts(&self, segment_id: i32) -> Result<(), WorkflowError>;
    async fn finish_training_backfill(&self, user_id: i32) -> Result<(), WorkflowError>;
    async fn prepare_archive(&self, job_id: i32) -> Result<ArchiveCursor, WorkflowError>;
    async fn archive_page(&self, cursor: ArchiveCursor) -> Result<ArchivePage, WorkflowError>;
    async fn finish_archive(&self, job_id: i32) -> Result<(), WorkflowError>;
    async fn pending_heatmaps(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError>;
    async fn pending_earned_awards(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError>;
    async fn pending_climbs(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError>;
    async fn activity_card_page(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError>;
    async fn personal_achievement_segments(&self, after_id: i32) -> Result<Vec<i32>, WorkflowError>;
    async fn rebuild_personal_achievements(&self, segment_id: i32) -> Result<(), WorkflowError>;
    async fn overall_achievement_segments(&self, after_id: i32) -> Result<Vec<i32>, WorkflowError>;
    async fn rebuild_overall_achievements(&self, segment_id: i32) -> Result<(), WorkflowError>;
}

pub struct BatchActivities {
    operations: Arc<dyn BatchOperations>,
}
impl BatchActivities {
    pub fn new(operations: Arc<dyn BatchOperations>) -> Self {
        Self { operations }
    }
}

#[activities]
impl BatchActivities {
    #[activity(name = "pending_earned_awards")]
    pub async fn pending_earned_awards(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move { self.operations.pending_earned_awards(after_id).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "activity_page")]
    pub async fn activity_page(
        self: Arc<Self>,
        _ctx: ActivityContext,
        input: ActivityPage,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move { self.operations.activity_page(input).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "reprocess_input")]
    pub async fn reprocess_input(
        self: Arc<Self>,
        _ctx: ActivityContext,
        input: ActivityTask,
    ) -> Result<ReprocessInput, ActivityError> {
        domain_task(async move { self.operations.reprocess_input(input).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "regenerate_segment_efforts")]
    pub async fn regenerate_segment_efforts(
        self: Arc<Self>,
        _ctx: ActivityContext,
        segment_id: i32,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.regenerate_segment_efforts(segment_id).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "finish_training_backfill")]
    pub async fn finish_training_backfill(
        self: Arc<Self>,
        _ctx: ActivityContext,
        user_id: i32,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.finish_training_backfill(user_id).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "prepare_archive")]
    pub async fn prepare_archive(
        self: Arc<Self>,
        _ctx: ActivityContext,
        job_id: i32,
    ) -> Result<ArchiveCursor, ActivityError> {
        domain_task(async move { self.operations.prepare_archive(job_id).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "archive_page")]
    pub async fn archive_page(
        self: Arc<Self>,
        _ctx: ActivityContext,
        cursor: ArchiveCursor,
    ) -> Result<ArchivePage, ActivityError> {
        domain_task(async move { self.operations.archive_page(cursor).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "finish_archive")]
    pub async fn finish_archive(self: Arc<Self>, _ctx: ActivityContext, job_id: i32) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.finish_archive(job_id).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "pending_climbs")]
    pub async fn pending_climbs(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move { self.operations.pending_climbs(after_id).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "activity_card_page")]
    pub async fn activity_card_page(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move { self.operations.activity_card_page(after_id).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "pending_heatmaps")]
    pub async fn pending_heatmaps(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move { self.operations.pending_heatmaps(after_id).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "personal_achievement_segments")]
    pub async fn personal_achievement_segments(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<i32>, ActivityError> {
        domain_task(async move { self.operations.personal_achievement_segments(after_id).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_personal_achievements")]
    pub async fn rebuild_personal_achievements(
        self: Arc<Self>,
        _ctx: ActivityContext,
        segment_id: i32,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_personal_achievements(segment_id).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "overall_achievement_segments")]
    pub async fn overall_achievement_segments(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<i32>, ActivityError> {
        domain_task(async move { self.operations.overall_achievement_segments(after_id).await })
            .await
            .map_err(domain_error)
    }

    #[activity(name = "rebuild_overall_achievements")]
    pub async fn rebuild_overall_achievements(
        self: Arc<Self>,
        _ctx: ActivityContext,
        segment_id: i32,
    ) -> Result<(), ActivityError> {
        domain_task(async move { self.operations.rebuild_overall_achievements(segment_id).await })
            .await
            .map_err(domain_error)
    }
}
