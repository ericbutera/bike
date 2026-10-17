//! Concrete Bike domain adapters. No SQL task queue or execution-stage records.
use crate::activities::{DerivedOperations, ImportOperations};
use bike_core::{
    bike_tasks::{ActivityImportResult, FitnessChange, HeatmapChange, ImportExecution, Publication, SegmentChange},
    object_store::ObjectStore,
    workflow_error::WorkflowError,
};
use sea_orm::DatabaseConnection;
use temporalio_sdk::activities::ActivityContext;

pub struct BikeOperations {
    db: DatabaseConnection,
    objects: ObjectStore,
}

impl BikeOperations {
    pub fn new(db: DatabaseConnection, objects: ObjectStore) -> Self {
        Self { db, objects }
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
    async fn rebuild_fitness_freshness(
        &self,
        change: FitnessChange,
        _context: ActivityContext,
    ) -> Result<Publication, WorkflowError> {
        bike_core::analytics::rebuild_fitness_freshness_cache(&self.db, change.user_id).await?;
        let state = bike_core::entities::analytics_user_states::Model::for_user(&self.db, change.user_id).await?;
        if state.is_some_and(|state| state.fitness_dirty_from_day.is_some()) {
            return Err(WorkflowError::internal(
                "Fitness changed during rebuild; retry current source",
            ));
        }
        Ok(Publication::Published)
    }

    async fn rebuild_segment_analytics(
        &self,
        change: SegmentChange,
        _context: ActivityContext,
    ) -> Result<Publication, WorkflowError> {
        use bike_core::entities::segments;
        let before = segments::Model::find_by_id(&self.db, change.segment_id).await?;
        bike_core::analytics::rebuild_segment_analytics_cache(&self.db, &[change.segment_id]).await?;
        let after = segments::Model::find_by_id(&self.db, change.segment_id).await?;
        if before != after {
            return Err(WorkflowError::internal(
                "Segment changed during rebuild; retry current source",
            ));
        }
        Ok(Publication::Published)
    }

    async fn prepare_heatmap(
        &self,
        change: HeatmapChange,
        _context: ActivityContext,
    ) -> Result<Publication, WorkflowError> {
        use bike_core::{
            entities::heatmap_projections,
            heatmaps::{
                preparation,
                projection::{PendingProjection, Projection},
            },
        };
        let Some(projection) = heatmap_projections::Model::for_activity(&self.db, change.activity_id).await? else {
            return Ok(Publication::Published);
        };
        if projection.user_id != change.user_id {
            return Err(WorkflowError::forbidden("Heatmap does not belong to this user"));
        }
        let pending = PendingProjection {
            activity_id: change.activity_id,
            generation: projection.generation,
        };
        if Projection::pending(&self.db, pending.activity_id, pending.generation).await? {
            preparation::prepare_activity(&self.db, pending).await?;
        }
        let current = heatmap_projections::Model::for_activity(&self.db, change.activity_id).await?;
        if current.is_some_and(|projection| matches!(projection.status.as_str(), "pending" | "failed")) {
            return Err(WorkflowError::internal(
                "Heatmap changed during preparation; rebuild current source",
            ));
        }
        Ok(Publication::Published)
    }
}
