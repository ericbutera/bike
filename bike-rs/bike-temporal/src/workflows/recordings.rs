//! Stage-only conversion of saved recordings; imports publish through the same model.
use super::options;
use crate::activities::{domain_error, domain_task};
use bike_core::{
    bike_tasks::{ActivityTask, Priority},
    entities::activity_recordings,
    object_store::ObjectStore,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use temporalio_macros::{activities, workflow, workflow_methods};
use temporalio_sdk::{
    activities::{ActivityContext, ActivityError},
    WorkflowContext, WorkflowResult,
};

pub struct RecordingActivities {
    db: DatabaseConnection,
    objects: ObjectStore,
}

impl RecordingActivities {
    pub fn new(db: DatabaseConnection, objects: ObjectStore) -> Self {
        Self { db, objects }
    }
}

#[activities]
impl RecordingActivities {
    #[activity(name = "pending_recordings")]
    pub async fn pending_recordings(
        self: Arc<Self>,
        _ctx: ActivityContext,
        after_id: i32,
    ) -> Result<Vec<ActivityTask>, ActivityError> {
        domain_task(async move {
            activity_recordings::Model::pending_tasks(&self.db, after_id)
                .await
                .map_err(Into::into)
        })
        .await
        .map_err(domain_error)
    }

    #[activity(name = "backfill_recording")]
    pub async fn backfill_recording(
        self: Arc<Self>,
        _ctx: ActivityContext,
        task: ActivityTask,
    ) -> Result<(), ActivityError> {
        domain_task(async move { activity_recordings::Model::backfill(&self.db, &self.objects, task).await })
            .await
            .map_err(domain_error)
    }
}

#[workflow]
#[derive(Default)]
pub struct BackfillActivityRecordings;

#[workflow_methods]
impl BackfillActivityRecordings {
    #[run(name = "backfill_activity_recordings")]
    pub async fn run(ctx: &mut WorkflowContext<Self>, mut after_id: i32) -> WorkflowResult<()> {
        let tasks = options::run(
            ctx,
            RecordingActivities::pending_recordings,
            after_id,
            options::activity(Priority::Background, "recording-backfill".into()),
        )
        .await?;
        let full = tasks.len() == 16;
        for task in tasks {
            let completed_id = task.activity_id;
            let fairness = format!("user-{}", task.user_id);
            options::run(
                ctx,
                RecordingActivities::backfill_recording,
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
    use temporalio_sdk::testing::ActivityEnvironment;
    use workflow_test_support::WorkflowMock;

    #[test]
    fn conversion_advances_only_after_saved_pages_and_empty_pages_finish() {
        let tasks: Vec<_> = (1..=16)
            .map(|activity_id| ActivityTask {
                activity_id,
                ..fixtures::activity_task()
            })
            .collect();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRecordings>(0);
        mock.activity("pending_recordings", &0_i32, &tasks);
        for task in &tasks {
            mock.activity("backfill_recording", task, &());
        }
        assert_eq!(mock.continued_input::<i32>(), 16);
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRecordings>(16);
        mock.activity("pending_recordings", &16_i32, &Vec::<ActivityTask>::new());
        mock.result::<()>();
    }

    #[test]
    fn failed_conversion_stops_before_later_records_and_partial_pages_finish() {
        let task = fixtures::activity_task();
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRecordings>(0);
        mock.activity("pending_recordings", &0_i32, &vec![task.clone(), task.clone()]);
        mock.fail_activity("backfill_recording", "missing legacy recording");
        mock.assert_failure_message("missing legacy recording");
        let mut mock: WorkflowMock = WorkflowMock::new::<BackfillActivityRecordings>(0);
        mock.activity("pending_recordings", &0_i32, &vec![task.clone()]);
        mock.activity("backfill_recording", &task, &());
        mock.result::<()>();
    }

    #[tokio::test]
    async fn conversion_activity_handlers_use_the_recording_model() {
        let row: std::collections::BTreeMap<String, sea_orm::Value> =
            [("activity_id".into(), 42.into()), ("user_id".into(), 7.into())].into();
        let activity = bike_core::entities::activities::Model {
            derived_data_json: Some(Default::default()),
            ..fixtures::activity()
        };
        let db = fixtures::postgres_mock()
            .append_query_results([vec![row]])
            .append_query_results([vec![activity]])
            .append_query_results([Vec::<activity_recordings::Model>::new()])
            .append_exec_results([fixtures::written()])
            .into_connection();
        let (objects, _) = fixtures::object_storage(bike_core::object_store::ObjectOwner::BikeTasks);
        let env = ActivityEnvironment::builder_with_default()
            .register_activities(RecordingActivities::new(db, objects))
            .build();
        let tasks = env.run(RecordingActivities::pending_recordings, 0).await.unwrap();
        assert_eq!(tasks.len(), 1);
        env.run(RecordingActivities::backfill_recording, tasks[0].clone())
            .await
            .unwrap();
    }
}
