use crate::activities::*;
use bike_core::{bike_tasks::*, test_support as fixtures, workflow_error::WorkflowError};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use temporalio_sdk::{activities::ActivityContext, testing::ActivityEnvironment};

#[derive(Default)]
pub(super) struct Operations {
    pub received: Mutex<Vec<(&'static str, serde_json::Value)>>,
    pub fail: bool,
}

impl Operations {
    pub fn expected<T>(&self, value: T) -> Result<T, ()> {
        if self.fail {
            Err(())
        } else {
            Ok(value)
        }
    }
    fn record(
        &self,
        name: &'static str,
        input: &impl Serialize,
        context: ActivityContext,
    ) -> Result<(), WorkflowError> {
        assert!(context.info().attempt > 0);
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

pub(super) async fn assert_activity<A>(
    environment: &ActivityEnvironment,
    definition: A,
    input: A::Input,
    expected: Result<A::Output, ()>,
) where
    A: temporalio_sdk::activities::ExecutableActivity,
    A::Output: PartialEq + std::fmt::Debug,
{
    let actual = environment.run(definition, input).await;
    match expected {
        Ok(value) => assert_eq!(actual.unwrap(), value),
        Err(()) => assert!(actual.is_err()),
    }
}

pub(super) fn environment(operations: Arc<Operations>) -> ActivityEnvironment {
    ActivityEnvironment::builder_with_default()
        .register_activities(ImportActivities::new(operations.clone()))
        .register_activities(DerivedActivities::new(operations.clone()))
        .register_activities(SegmentActivities::new(operations.clone()))
        .register_activities(TrainingActivities::new(operations.clone()))
        .register_activities(ArchiveActivities::new(operations))
        .build()
}

#[async_trait::async_trait]
impl ImportOperations for Operations {
    async fn activity_import(
        &self,
        command: ActivityImport,
        context: ActivityContext,
    ) -> Result<ActivityImportResult, WorkflowError> {
        self.record("activity_import", &command, context)?;
        Ok(fixtures::import_result())
    }
    async fn prepare_reprocess_imports(
        &self,
        selection: ReprocessSelection,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError> {
        self.record("prepare_reprocess_imports", &selection, context)?;
        Ok(fixtures::batch_manifest(
            &selection.command,
            ManifestScope::Reprocessing {
                user_id: selection.command.user_id,
                selection: selection.scope,
            },
            1,
        ))
    }
    async fn reprocess_import_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<ReprocessInput>, WorkflowError> {
        self.record("reprocess_import_page", &page, context)?;
        Ok((page.offset..page.offset + page.expected_entries() as u64)
            .map(|index| fixtures::reprocess_entry(&page, index))
            .collect())
    }
}

#[async_trait::async_trait]
impl DerivedOperations for Operations {
    async fn rebuild_fitness_freshness(
        &self,
        change: FitnessChange,
        context: ActivityContext,
    ) -> Result<Publication, WorkflowError> {
        self.record("rebuild_fitness_freshness", &change, context)?;
        Ok(Publication::Published)
    }
    async fn rebuild_segment_analytics(
        &self,
        change: SegmentChange,
        context: ActivityContext,
    ) -> Result<Publication, WorkflowError> {
        self.record("rebuild_segment_analytics", &change, context)?;
        Ok(Publication::Published)
    }
    async fn prepare_heatmap(
        &self,
        change: HeatmapChange,
        context: ActivityContext,
    ) -> Result<Publication, WorkflowError> {
        self.record("prepare_heatmap", &change, context)?;
        Ok(Publication::Published)
    }
}

#[async_trait::async_trait]
impl SegmentOperations for Operations {
    async fn match_activity_segments(
        &self,
        command: ActivitySegmentsInput,
        context: ActivityContext,
    ) -> Result<ActivitySegmentsResult, WorkflowError> {
        self.record("match_activity_segments", &command, context)?;
        Ok(ActivitySegmentsResult {
            activity_id: command.activity_id,
            ..fixtures::segment_result()
        })
    }
    async fn prepare_user_segments(
        &self,
        command: BatchCommand,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError> {
        self.record("prepare_user_segments", &command, context)?;
        Ok(fixtures::batch_manifest(
            &command,
            ManifestScope::UserSegments {
                user_id: command.user_id,
            },
            1,
        ))
    }
    async fn prepare_segment_efforts(
        &self,
        selection: SegmentEffortsSelection,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError> {
        self.record("prepare_segment_efforts", &selection, context)?;
        Ok(fixtures::batch_manifest(
            &selection.command,
            ManifestScope::Segment {
                segment_id: selection.segment_id,
                expected_revision: selection.expected_revision,
            },
            1,
        ))
    }
    async fn segment_activity_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<ActivitySegmentsInput>, WorkflowError> {
        self.record("segment_activity_page", &page, context)?;
        Ok((page.offset..page.offset + page.expected_entries() as u64)
            .map(|index| fixtures::segment_entry(&page, index))
            .collect())
    }
}

#[async_trait::async_trait]
impl TrainingOperations for Operations {
    async fn prepare_training_backfill(
        &self,
        command: BatchCommand,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError> {
        self.record("prepare_training_backfill", &command, context)?;
        Ok(fixtures::batch_manifest(
            &command,
            ManifestScope::Training {
                user_id: command.user_id,
            },
            1,
        ))
    }
    async fn training_activity_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<TrainingActivityInput>, WorkflowError> {
        self.record("training_activity_page", &page, context)?;
        Ok((page.offset..page.offset + page.expected_entries() as u64)
            .map(|index| fixtures::training_entry(&page, index))
            .collect())
    }
    async fn rebuild_activity_training(
        &self,
        command: TrainingActivityInput,
        context: ActivityContext,
    ) -> Result<TrainingActivityResult, WorkflowError> {
        self.record("rebuild_activity_training", &command, context)?;
        Ok(fixtures::training_result())
    }
    async fn complete_training_backfill(
        &self,
        completion: TrainingCompletion,
        context: ActivityContext,
    ) -> Result<(), WorkflowError> {
        self.record("complete_training_backfill", &completion, context)
    }
}

#[async_trait::async_trait]
impl ArchiveOperations for Operations {
    async fn prepare_archive_manifest(
        &self,
        command: ArchiveCommand,
        context: ActivityContext,
    ) -> Result<BatchManifest, WorkflowError> {
        self.record("prepare_archive_manifest", &command, context)?;
        Ok(fixtures::batch_manifest(
            &command.batch,
            ManifestScope::Archive {
                user_id: command.batch.user_id,
                archive_job_id: command.archive_job_id,
            },
            1,
        ))
    }
    async fn archive_import_page(
        &self,
        page: PageRequest,
        context: ActivityContext,
    ) -> Result<Vec<ImportSource>, WorkflowError> {
        self.record("archive_import_page", &page, context)?;
        Ok((page.offset..page.offset + page.expected_entries() as u64)
            .map(|index| fixtures::import_entry(&page, index))
            .collect())
    }
}
