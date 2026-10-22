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

    async fn pending_climbs(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError> {
        Ok(bike_core::entities::activity_climbs::output::Model::pending_tasks(&self.db, after_id).await?)
    }

    async fn pending_heatmaps(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError> {
        bike_core::heatmaps::projection::Projection::pending_tasks(&self.db, after_id)
            .await
            .map_err(Into::into)
    }

    async fn personal_achievement_segments(&self, after_id: i32) -> Result<Vec<i32>, WorkflowError> {
        Ok(bike_core::entities::segments::Model::ids_page(&self.db, after_id, 16).await?)
    }

    async fn rebuild_personal_achievements(&self, segment_id: i32) -> Result<(), WorkflowError> {
        bike_core::personal_achievements::rebuild(&self.db, segment_id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activities::BatchActivities;
    use bike_core::{object_store::ObjectOwner, test_support as fixtures};
    use std::{collections::BTreeMap, sync::Arc};
    use temporalio_sdk::testing::ActivityEnvironment;

    #[tokio::test]
    async fn registered_backfill_handlers_select_segments_and_publish_personal_achievements() {
        let row: BTreeMap<String, sea_orm::Value> = [("id".into(), fixtures::segment().id.into())].into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![row]])
            .append_query_results([vec![fixtures::segment()]])
            .append_query_results([vec![fixtures::segment_effort()]])
            .append_exec_results((0..2).map(|_| fixtures::written()))
            .into_connection();
        let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
        let environment = ActivityEnvironment::builder_with_default()
            .register_activities(BatchActivities::new(Arc::new(BikeOperations::new(db, objects))))
            .build();
        let ids = environment
            .run(BatchActivities::personal_achievement_segments, 0)
            .await
            .unwrap();
        assert_eq!(ids, vec![fixtures::segment().id]);
        environment
            .run(BatchActivities::rebuild_personal_achievements, ids[0])
            .await
            .unwrap();
    }
}
