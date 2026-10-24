//! Activities call domain operations directly; Temporal owns their execution state.
use crate::activities::{DerivedOperations, ImportOperations};
use bike_core::{
    bike_tasks::{ActivityImportResult, ActivityTask, FitnessChange, ImportExecution, SegmentChange},
    entities::{activities, heatmap_projections},
    heatmaps::{
        preparation,
        projection::{PendingProjection, Projection},
    },
    object_store::ObjectStore,
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use temporalio_sdk::activities::ActivityContext;
mod batches;

pub struct BikeOperations {
    db: DatabaseConnection,
    objects: ObjectStore,
}

impl BikeOperations {
    pub fn new(db: DatabaseConnection, objects: ObjectStore) -> Self {
        Self { db, objects }
    }

    async fn activity(&self, task: &ActivityTask) -> Result<activities::Model, WorkflowError> {
        activities::Model::find_owned(&self.db, task.activity_id, task.user_id)
            .await?
            .ok_or_else(|| WorkflowError::not_found("Activity not found"))
    }
}

#[async_trait::async_trait]
impl ImportOperations for BikeOperations {
    async fn activity_import(
        &self,
        execution: ImportExecution,
        _context: ActivityContext,
    ) -> Result<ActivityImportResult, WorkflowError> {
        bike_core::activity_reprocessing::reprocess(&self.db, &self.objects, execution).await
    }
}

#[async_trait::async_trait]
impl DerivedOperations for BikeOperations {
    async fn rebuild_activity_segments(&self, task: ActivityTask) -> Result<Vec<i32>, WorkflowError> {
        let activity = self.activity(&task).await?;
        bike_core::segment_regeneration::match_activity(&self.db, &activity).await
    }

    async fn rebuild_activity_achievements(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        let activity = self.activity(&task).await?;
        bike_core::activity_achievements::record(&self.db, &activity).await?;
        bike_core::analytics::rebuild_activity_analytics_cache(&self.db, &[activity.id]).await?;
        Ok(())
    }

    async fn rebuild_activity_training(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        let activity = self.activity(&task).await?;
        if activity.is_bike_activity() {
            bike_core::activity_training_analysis::rebuild_activity_training_analysis_cache(&self.db, &[activity.id])
                .await?;
        } else {
            bike_core::entities::activity_training_analyses::Model::clear_activity(&self.db, activity.id).await?;
        }
        Ok(())
    }

    async fn rebuild_activity_climbs(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        bike_core::activity_climbs::rebuild(&self.db, &self.objects, task).await
    }

    async fn rebuild_activity_card(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        bike_core::activity_cards::rebuild(&self.db, task).await
    }

    async fn rebuild_fitness_freshness(&self, task: FitnessChange) -> Result<(), WorkflowError> {
        bike_core::analytics::rebuild_fitness_freshness_cache(&self.db, task.user_id).await?;
        Ok(())
    }

    async fn rebuild_segment_analytics(&self, task: SegmentChange) -> Result<(), WorkflowError> {
        bike_core::analytics::rebuild_segment_analytics_cache(&self.db, &[task.segment_id]).await?;
        Ok(())
    }

    async fn prepare_heatmap(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        self.activity(&task).await?;
        let Some(projection) = heatmap_projections::Model::for_activity(&self.db, task.activity_id).await? else {
            return Ok(());
        };
        if projection.user_id != task.user_id {
            return Err(WorkflowError::forbidden("Heatmap does not belong to this user"));
        }
        let pending = PendingProjection {
            activity_id: task.activity_id,
            generation: projection.generation,
        };
        if Projection::pending(&self.db, pending.activity_id, pending.generation).await? {
            preparation::prepare_activity(&self.db, pending).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activities::{BatchActivities, DerivedActivities};
    use bike_core::{object_store::ObjectOwner, test_support as fixtures};
    use sea_orm::Value;
    use std::{collections::BTreeMap, sync::Arc};
    use temporalio_sdk::testing::ActivityEnvironment;

    #[tokio::test]
    async fn backfill_and_import_activity_handlers_share_the_persisted_climb_operation() {
        let activity = fixtures::activity();
        let pending: BTreeMap<String, Value> = [
            ("activity_id".into(), activity.id.into()),
            ("user_id".into(), activity.user_id.into()),
        ]
        .into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![pending]])
            .append_query_results([vec![activity.clone()]])
            .append_query_results([Vec::<bike_core::entities::activity_climbs::output::Model>::new()])
            .append_query_results([vec![activity]])
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
        let operations = Arc::new(BikeOperations::new(db, objects));
        let environment = ActivityEnvironment::builder_with_default()
            .register_activities(BatchActivities::new(operations.clone()))
            .register_activities(DerivedActivities::new(operations))
            .build();
        let tasks = environment.run(BatchActivities::pending_climbs, 0).await.unwrap();
        assert_eq!(tasks.len(), 1);
        environment
            .run(DerivedActivities::rebuild_activity_climbs, tasks[0].clone())
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn backfill_and_import_activity_handlers_share_the_persisted_card_operation() {
        let activity = fixtures::activity();
        let pending: BTreeMap<String, Value> = [
            ("activity_id".into(), activity.id.into()),
            ("user_id".into(), activity.user_id.into()),
        ]
        .into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![pending]])
            .append_query_results([vec![activity.clone()]])
            .append_query_results([vec![activity]])
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
        let operations = Arc::new(BikeOperations::new(db, objects));
        let environment = ActivityEnvironment::builder_with_default()
            .register_activities(BatchActivities::new(operations.clone()))
            .register_activities(DerivedActivities::new(operations))
            .build();
        let tasks = environment.run(BatchActivities::activity_card_page, 0).await.unwrap();
        assert_eq!(tasks.len(), 1);
        environment
            .run(DerivedActivities::rebuild_activity_card, tasks[0].clone())
            .await
            .unwrap();
    }
}
