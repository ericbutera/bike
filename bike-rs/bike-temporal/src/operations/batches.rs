use super::BikeOperations;
use crate::activities::BatchOperations;
use bike_core::{bike_tasks::*, entities::activities, workflow_error::WorkflowError};

#[async_trait::async_trait]
impl BatchOperations for BikeOperations {
    async fn activity_page(&self, input: ActivityPage) -> Result<Vec<ActivityTask>, WorkflowError> {
        let page = input.page;
        let ids = match input.selection {
            ActivitySelection::Cycling => {
                activities::Model::cycling_ids_page(&self.db, page.user_id, page.after_id).await?
            }
            ActivitySelection::Imports { archive_fits_only } => {
                activities::Model::imported_ids_page(&self.db, page.user_id, page.after_id, archive_fits_only).await?
            }
        };
        Ok(ids
            .into_iter()
            .map(|activity_id| ActivityTask {
                user_id: page.user_id,
                activity_id,
                priority: Priority::Background,
            })
            .collect())
    }

    async fn reprocess_input(&self, task: ActivityTask) -> Result<ReprocessInput, WorkflowError> {
        self.activity(&task).await?;
        bike_core::activity_reprocessing::command(
            &self.db,
            task.activity_id,
            uuid::Uuid::new_v4().to_string(),
            task.priority,
        )
        .await
    }

    async fn regenerate_segment_efforts(&self, segment_id: i32) -> Result<(), WorkflowError> {
        bike_core::segment_regeneration::regenerate_segment_efforts(&self.db, segment_id).await?;
        Ok(())
    }

    async fn finish_training_backfill(&self, user_id: i32) -> Result<(), WorkflowError> {
        bike_core::xc_goal_backfill::mark_user_xc_goal_backfill_completed(&self.db, user_id, chrono::Utc::now())
            .await?;
        Ok(())
    }

    async fn prepare_archive(&self, job_id: i32) -> Result<ArchiveCursor, WorkflowError> {
        bike_core::archive_import::native::prepare(&self.db, &self.objects, job_id).await
    }

    async fn archive_page(&self, cursor: ArchiveCursor) -> Result<ArchivePage, WorkflowError> {
        bike_core::archive_import::native::page(&self.db, &self.objects, cursor).await
    }

    async fn finish_archive(&self, job_id: i32) -> Result<(), WorkflowError> {
        bike_core::archive_import::native::finish(&self.db, job_id).await
    }

    async fn pending_heatmaps(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError> {
        bike_core::heatmaps::projection::Projection::pending_tasks(&self.db, after_id)
            .await
            .map_err(Into::into)
    }
}
