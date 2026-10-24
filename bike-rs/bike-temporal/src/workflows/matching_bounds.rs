//! Focused bounds backfills; matching and standings are separate operations.
use super::options;
use crate::activities::{domain_error, domain_task};
use bike_core::{
    bike_tasks::{ActivityTask, Priority},
    entities::route_matching_bounds,
    object_store::ObjectStore,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::{
    activities::{ActivityContext, ActivityError},
    WorkflowContext, WorkflowResult,
};

pub struct BoundsActivities {
    db: DatabaseConnection,
    objects: ObjectStore,
}
impl BoundsActivities {
    pub fn new(db: DatabaseConnection, objects: ObjectStore) -> Self {
        Self { db, objects }
    }
}
#[activities]
impl BoundsActivities {
    #[activity(name = "pending_activity_bounds")]
    pub async fn pending_activities(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move {
            route_matching_bounds::Model::pending_activities(&self.db, after_id)
                .await
                .map_err(Into::into)
        })
        .await
        .map_err(domain_error)
    }
    #[activity(name = "pending_segment_bounds")]
    pub async fn pending_segments(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<i32>, ActivityError> {
        domain_task(async move {
            route_matching_bounds::Model::pending_segments(&self.db, after_id)
                .await
                .map_err(Into::into)
        })
        .await
        .map_err(domain_error)
    }
    #[activity(name = "save_activity_bounds")]
    pub async fn save_activity(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { route_matching_bounds::Model::backfill_activity(&self.db, &self.objects, task).await })
            .await
            .map_err(domain_error)
    }
    #[activity(name = "save_segment_bounds")]
    pub async fn save_segment(self: Arc<Self>, _ctx: ActivityContext, segment_id: i32) -> Result<(), ActivityError> {
        domain_task(async move { route_matching_bounds::Model::backfill_segment(&self.db, segment_id).await })
            .await
            .map_err(domain_error)
    }
}

#[workflow]
#[derive(Default)]
pub struct BackfillActivityRouteBounds;
#[workflow_methods]
impl BackfillActivityRouteBounds {
    #[run(name = "backfill_activity_route_bounds")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let tasks = options::run(
            ctx,
            BoundsActivities::pending_activities,
            after_id,
            options::activity(Priority::Background, "activity-bounds".into()),
        )
        .await?;
        let full = tasks.len() == 16;
        for task in tasks {
            let completed_id = task.activity_id;
            let fairness = format!("user-{}", task.user_id);
            options::run(
                ctx,
                BoundsActivities::save_activity,
                task,
                options::activity(Priority::Background, fairness),
            )
            .await?;
            after_id = completed_id;
        }
        if full {
            return ctx
                .continue_as_new(after_id, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}
#[workflow]
#[derive(Default)]
pub struct BackfillSegmentRouteBounds;
#[workflow_methods]
impl BackfillSegmentRouteBounds {
    #[run(name = "backfill_segment_route_bounds")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let ids = options::run(
            ctx,
            BoundsActivities::pending_segments,
            after_id,
            options::activity(Priority::Background, "segment-bounds".into()),
        )
        .await?;
        let full = ids.len() == 16;
        for segment_id in ids {
            options::run(
                ctx,
                BoundsActivities::save_segment,
                segment_id,
                options::activity(Priority::Background, format!("segment-{segment_id}")),
            )
            .await?;
            after_id = segment_id;
        }
        if full {
            return ctx
                .continue_as_new(after_id, Default::default())
                .map(|never| match never {});
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::test_support as fixtures;
    use workflow_test_support::WorkflowMock;

    #[test]
    fn activity_bounds_pages_advance_after_saved_outputs_and_missing_recording_stops_progress() {
        let tasks: Vec<_> = (1..=16)
            .map(|activity_id| ActivityTask {
                activity_id,
                ..fixtures::activity_task()
            })
            .collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRouteBounds>(0);
        mock.activity("pending_activity_bounds", &0_i32, &tasks);
        for task in &tasks {
            mock.activity("save_activity_bounds", task, &());
        }
        assert_eq!(mock.continued_input::<i32>(), 16);
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRouteBounds>(16);
        mock.activity("pending_activity_bounds", &16_i32, &Vec::<ActivityTask>::new());
        mock.result::<()>();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRouteBounds>(0);
        mock.activity("pending_activity_bounds", &0_i32, &tasks);
        mock.fail_activity("save_activity_bounds", "normalized recording missing");
        mock.assert_failure_message("normalized recording missing");
    }

    #[test]
    fn segment_bounds_pages_keep_the_last_saved_id_and_fail_before_advancing() {
        let ids: Vec<i32> = (1..=16).collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillSegmentRouteBounds>(0);
        mock.activity("pending_segment_bounds", &0_i32, &ids);
        for id in &ids {
            mock.activity("save_segment_bounds", id, &());
        }
        assert_eq!(mock.continued_input::<i32>(), 16);
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillSegmentRouteBounds>(16);
        mock.activity("pending_segment_bounds", &16_i32, &vec![17]);
        mock.activity("save_segment_bounds", &17_i32, &());
        mock.result::<()>();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillSegmentRouteBounds>(0);
        mock.activity("pending_segment_bounds", &0_i32, &ids);
        mock.fail_activity("save_segment_bounds", "segment write failed");
        mock.assert_failure_message("segment write failed");
    }

    #[tokio::test]
    async fn registered_bounds_handlers_use_normalized_recordings_and_fixed_segments() {
        use bike_core::{
            entities::{activity_recordings, segments},
            object_store::ObjectOwner,
        };
        use temporalio_sdk::testing::ActivityEnvironment;
        let (objects, _) = fixtures::object_storage(ObjectOwner::BikeTasks);
        let recording = activity_recordings::Recording::from(fixtures::parsed_reprocess_activity().derived_data);
        let reference = objects.transform("bike/tasks/recordings/7", &recording).await.unwrap();
        let activity_row: std::collections::BTreeMap<String, sea_orm::Value> =
            [("activity_id".into(), 42.into()), ("user_id".into(), 7.into())].into();
        let segment_row: std::collections::BTreeMap<String, sea_orm::Value> = [("id".into(), 1.into())].into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![activity_row]])
            .append_query_results([vec![segment_row]])
            .append_query_results([vec![fixtures::activity()]])
            .append_query_results([vec![fixtures::activity_recording(&reference)]])
            .append_query_results([vec![segments::Model {
                id: 1,
                ..fixtures::segment()
            }]])
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        let environment = ActivityEnvironment::builder_with_default()
            .register_activities(BoundsActivities::new(db, objects))
            .build();
        let tasks = environment.run(BoundsActivities::pending_activities, 0).await.unwrap();
        let ids = environment.run(BoundsActivities::pending_segments, 0).await.unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(ids, vec![1]);
        environment
            .run(BoundsActivities::save_activity, tasks[0].clone())
            .await
            .unwrap();
        environment.run(BoundsActivities::save_segment, ids[0]).await.unwrap();
    }
}
