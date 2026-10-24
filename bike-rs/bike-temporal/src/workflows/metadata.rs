//! Focused conversion of saved metadata and laps without replaying imports.
use super::options;
use crate::activities::{domain_error, domain_task};
use bike_core::{
    bike_tasks::{ActivityTask, Priority},
    entities::activity_metadata,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::{
    activities::{ActivityContext, ActivityError},
    WorkflowContext, WorkflowResult,
};

pub struct MetadataActivities {
    db: DatabaseConnection,
}
impl MetadataActivities {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}
#[activities]
impl MetadataActivities {
    #[activity(name = "pending_activity_metadata")]
    pub async fn pending(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move {
            activity_metadata::Model::pending_tasks(&self.db, after_id)
                .await
                .map_err(Into::into)
        })
        .await
        .map_err(domain_error)
    }

    #[activity(name = "backfill_activity_metadata_row")]
    pub async fn backfill(self: Arc<Self>, _ctx: ActivityContext, task: ActivityTask) -> Result<(), ActivityError> {
        domain_task(async move { activity_metadata::Model::backfill(&self.db, task).await })
            .await
            .map_err(domain_error)
    }
}

#[workflow]
#[derive(Default)]
pub struct BackfillActivityMetadata;
#[workflow_methods]
impl BackfillActivityMetadata {
    #[run(name = "backfill_activity_metadata")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let tasks = options::run(
            ctx,
            MetadataActivities::pending,
            after_id,
            options::activity(Priority::Background, "metadata-backfill".into()),
        )
        .await?;
        let full = tasks.len() == 16;
        for task in tasks {
            let completed_id = task.activity_id;
            let fairness = format!("user-{}", task.user_id);
            options::run(
                ctx,
                MetadataActivities::backfill,
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

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::test_support as fixtures;
    use workflow_test_support::WorkflowMock;

    #[test]
    fn metadata_pages_advance_only_after_saved_rows_and_empty_pages_finish() {
        let tasks: Vec<_> = (1..=16)
            .map(|activity_id| ActivityTask {
                activity_id,
                ..fixtures::activity_task()
            })
            .collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityMetadata>(0);
        mock.activity("pending_activity_metadata", &0_i32, &tasks);
        for task in &tasks {
            mock.activity("backfill_activity_metadata_row", task, &());
        }
        assert_eq!(mock.continued_input::<i32>(), 16);
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityMetadata>(16);
        mock.activity("pending_activity_metadata", &16_i32, &Vec::<ActivityTask>::new());
        mock.result::<()>();
    }

    #[test]
    fn metadata_failure_stops_before_advancing_and_partial_pages_finish() {
        let task = fixtures::activity_task();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityMetadata>(0);
        mock.activity("pending_activity_metadata", &0_i32, &vec![task.clone(), task.clone()]);
        mock.fail_activity("backfill_activity_metadata_row", "metadata unavailable");
        mock.assert_failure_message("metadata unavailable");
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityMetadata>(0);
        mock.activity("pending_activity_metadata", &0_i32, &vec![task.clone()]);
        mock.activity("backfill_activity_metadata_row", &task, &());
        mock.result::<()>();
    }

    #[tokio::test]
    async fn registered_metadata_handlers_use_the_owned_saved_activity_producer() {
        use temporalio_sdk::testing::ActivityEnvironment;
        let row: std::collections::BTreeMap<String, sea_orm::Value> =
            [("activity_id".into(), 42.into()), ("user_id".into(), 7.into())].into();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![row]])
            .append_query_results([vec![fixtures::activity()]])
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        let environment = ActivityEnvironment::builder_with_default()
            .register_activities(MetadataActivities::new(db))
            .build();
        let tasks = environment.run(MetadataActivities::pending, 0).await.unwrap();
        assert_eq!(
            tasks,
            vec![ActivityTask {
                priority: Priority::Background,
                ..fixtures::activity_task()
            }]
        );
        environment
            .run(MetadataActivities::backfill, tasks[0].clone())
            .await
            .unwrap();
    }
}
