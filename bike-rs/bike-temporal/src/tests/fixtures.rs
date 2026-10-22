use crate::activities::{
    BatchActivities, BatchOperations, DerivedActivities, DerivedOperations, ImportActivities, ImportOperations,
};
use bike_core::{bike_tasks::*, test_support as fixtures, workflow_error::WorkflowError};
use std::sync::{Arc, Mutex};
use temporalio_sdk::{activities::ActivityContext, testing::ActivityEnvironment};

#[derive(Default)]
pub(super) struct Operations {
    pub received: Mutex<Vec<(&'static str, serde_json::Value)>>,
    pub fail: bool,
}

impl Operations {
    fn record(&self, name: &'static str, input: &impl serde::Serialize) -> Result<(), WorkflowError> {
        self.received
            .lock()
            .unwrap()
            .push((name, serde_json::to_value(input).unwrap()));
        if self.fail {
            Err(WorkflowError::internal("fixture failed"))
        } else {
            Ok(())
        }
    }
}

pub(super) fn environment(operations: Arc<Operations>) -> ActivityEnvironment {
    let job = fixtures::job_reference();
    ActivityEnvironment::builder_with_default()
        .info(
            temporalio_sdk::testing::TestActivityInfoOptions::builder()
                .namespace(job.namespace)
                .workflow_id(Some(job.job_id))
                .workflow_run_id(Some(job.run_id))
                .workflow_type(Some(job.workflow_type))
                .build(),
        )
        .register_activities(ImportActivities::new(operations.clone()))
        .register_activities(BatchActivities::new(operations.clone()))
        .register_activities(DerivedActivities::new(operations))
        .build()
}

#[async_trait::async_trait]
impl BatchOperations for Operations {
    async fn activity_page(&self, input: ActivityPage) -> Result<Vec<ActivityTask>, WorkflowError> {
        self.record("activity_page", &input)?;
        Ok(vec![fixtures::activity_task()])
    }
    async fn reprocess_input(&self, input: ActivityTask) -> Result<ReprocessInput, WorkflowError> {
        self.record("reprocess_input", &input)?;
        Ok(fixtures::reprocess_input())
    }
    async fn regenerate_segment_efforts(&self, segment_id: i32) -> Result<(), WorkflowError> {
        self.record("regenerate_segment_efforts", &segment_id)
    }
    async fn finish_training_backfill(&self, user_id: i32) -> Result<(), WorkflowError> {
        self.record("finish_training_backfill", &user_id)
    }
    async fn prepare_archive(&self, job_id: i32) -> Result<ArchiveCursor, WorkflowError> {
        self.record("prepare_archive", &job_id)?;
        Ok(ArchiveCursor {
            job_id,
            archive_path: "archives/7.zip".into(),
            offset: 0,
        })
    }
    async fn archive_page(&self, cursor: ArchiveCursor) -> Result<ArchivePage, WorkflowError> {
        self.record("archive_page", &cursor)?;
        Ok(ArchivePage {
            sources: vec![fixtures::import_command().source],
            next_offset: 1,
            done: true,
        })
    }
    async fn finish_archive(&self, job_id: i32) -> Result<(), WorkflowError> {
        self.record("finish_archive", &job_id)
    }
    async fn personal_achievement_segments(&self, after_id: i32) -> Result<Vec<i32>, WorkflowError> {
        self.record("personal_achievement_segments", &after_id)?;
        Ok(vec![fixtures::segment().id])
    }
    async fn rebuild_personal_achievements(&self, segment_id: i32) -> Result<(), WorkflowError> {
        self.record("rebuild_personal_achievements", &segment_id)
    }
    async fn pending_climbs(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError> {
        self.record("pending_climbs", &after_id)?;
        Ok(vec![fixtures::activity_task()])
    }
    async fn pending_heatmaps(&self, after_id: i32) -> Result<Vec<ActivityTask>, WorkflowError> {
        self.record("pending_heatmaps", &after_id)?;
        Ok(vec![fixtures::activity_task()])
    }
}

#[async_trait::async_trait]
impl ImportOperations for Operations {
    async fn activity_import(
        &self,
        execution: ImportExecution,
        _context: ActivityContext,
    ) -> Result<ActivityImportResult, WorkflowError> {
        self.record("activity_import", &execution.command)?;
        Ok(fixtures::import_result())
    }
}

#[async_trait::async_trait]
impl DerivedOperations for Operations {
    async fn rebuild_activity_segments(&self, task: ActivityTask) -> Result<Vec<i32>, WorkflowError> {
        self.record("rebuild_activity_segments", &task)?;
        Ok(vec![18])
    }
    async fn rebuild_activity_achievements(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        self.record("rebuild_activity_achievements", &task)
    }
    async fn rebuild_fitness_freshness(&self, task: FitnessChange) -> Result<(), WorkflowError> {
        self.record("rebuild_fitness_freshness", &task)
    }
    async fn rebuild_activity_climbs(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        self.record("rebuild_activity_climbs", &task)
    }
    async fn rebuild_activity_training(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        self.record("rebuild_activity_training", &task)
    }
    async fn rebuild_segment_analytics(&self, task: SegmentChange) -> Result<(), WorkflowError> {
        self.record("rebuild_segment_analytics", &task)
    }
    async fn prepare_heatmap(&self, task: ActivityTask) -> Result<(), WorkflowError> {
        self.record("prepare_heatmap", &task)
    }
}

pub(super) fn assert_retryable(error: &temporalio_sdk::testing::ActivityEnvironmentError) {
    use temporalio_sdk::{activities::ActivityError, testing::ActivityEnvironmentError};
    let ActivityEnvironmentError::Activity(ActivityError::Application(failure)) = error else {
        panic!("Expected native application failure: {error:?}");
    };
    assert!(!failure.is_non_retryable());
}
