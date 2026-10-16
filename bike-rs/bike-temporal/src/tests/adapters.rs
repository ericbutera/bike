use super::fixtures::{assert_activity, environment, Operations};
use crate::activities::*;
use bike_core::{bike_tasks::*, test_support as fixtures};
use std::sync::Arc;

#[tokio::test]
async fn reprocess_manifest_and_page_adapters_preserve_selection_inputs_and_failures() {
    let request = fixtures::page_request(
        ManifestScope::Reprocessing {
            user_id: 7,
            selection: ReprocessScope::ArchiveFits,
        },
        1,
        0,
    );
    let selection = ReprocessSelection {
        command: request.command.clone(),
        scope: ReprocessScope::ArchiveFits,
    };
    for fail in [false, true] {
        let operations = Arc::new(Operations {
            fail,
            ..Default::default()
        });
        let environment = environment(operations.clone());
        assert_activity(
            &environment,
            ReprocessManifestActivities::prepare_reprocess_imports,
            selection.clone(),
            operations.expected(request.manifest.clone()),
        )
        .await;
        assert_activity(
            &environment,
            ReprocessManifestActivities::reprocess_import_page,
            request.clone(),
            operations.expected(vec![fixtures::reprocess_entry(&request, 0)]),
        )
        .await;
        assert_eq!(
            operations.received.lock().unwrap()[0].1,
            serde_json::to_value(&selection).unwrap()
        );
    }
}

#[tokio::test]
async fn segment_adapters_preserve_user_and_global_scope_and_errors() {
    let user_page = fixtures::page_request(ManifestScope::UserSegments { user_id: 7 }, 1, 0);
    let segment_page = fixtures::page_request(
        ManifestScope::Segment {
            segment_id: 18,
            expected_revision: 1,
        },
        1,
        0,
    );
    let selection = SegmentEffortsSelection {
        command: fixtures::batch_command(),
        segment_id: 18,
        expected_revision: 1,
    };
    for fail in [false, true] {
        let operations = Arc::new(Operations {
            fail,
            ..Default::default()
        });
        let environment = environment(operations.clone());
        assert_activity(
            &environment,
            SegmentActivities::match_activity_segments,
            fixtures::segment_entry(&user_page, 0),
            operations.expected(fixtures::segment_result()),
        )
        .await;
        assert_activity(
            &environment,
            SegmentActivities::prepare_user_segments,
            user_page.command.clone(),
            operations.expected(user_page.manifest.clone()),
        )
        .await;
        assert_activity(
            &environment,
            SegmentActivities::prepare_segment_efforts,
            selection.clone(),
            operations.expected(segment_page.manifest.clone()),
        )
        .await;
        assert_activity(
            &environment,
            SegmentActivities::segment_activity_page,
            segment_page.clone(),
            operations.expected(vec![fixtures::segment_entry(&segment_page, 0)]),
        )
        .await;
        assert_eq!(operations.received.lock().unwrap().len(), 4);
    }
}

#[tokio::test]
async fn training_adapters_preserve_activity_inputs_and_completion_failure() {
    let request = fixtures::page_request(ManifestScope::Training { user_id: 7 }, 1, 0);
    let completion = TrainingCompletion {
        command: request.command.clone(),
        result: BatchResult {
            command_id: request.command.command_id.clone(),
            completed: 1,
        },
    };
    for fail in [false, true] {
        let operations = Arc::new(Operations {
            fail,
            ..Default::default()
        });
        let environment = environment(operations.clone());
        assert_activity(
            &environment,
            TrainingActivities::prepare_training_backfill,
            request.command.clone(),
            operations.expected(request.manifest.clone()),
        )
        .await;
        assert_activity(
            &environment,
            TrainingActivities::training_activity_page,
            request.clone(),
            operations.expected(vec![fixtures::training_entry(&request, 0)]),
        )
        .await;
        assert_activity(
            &environment,
            TrainingActivities::rebuild_activity_training,
            fixtures::training_entry(&request, 0),
            operations.expected(fixtures::training_result()),
        )
        .await;
        assert_activity(
            &environment,
            TrainingActivities::complete_training_backfill,
            completion.clone(),
            operations.expected(()),
        )
        .await;
        assert_eq!(
            operations.received.lock().unwrap()[3].1,
            serde_json::to_value(&completion).unwrap()
        );
    }
}

#[tokio::test]
async fn archive_adapters_preserve_retained_sources_and_page_failures() {
    let input = fixtures::archive_input();
    let request = fixtures::page_request(
        ManifestScope::Archive {
            user_id: 7,
            archive_job_id: 9,
        },
        1,
        0,
    );
    let command = ArchiveCommand {
        batch: input.batch.command,
        archive_job_id: input.archive_job_id,
        source: input.source,
    };
    for fail in [false, true] {
        let operations = Arc::new(Operations {
            fail,
            ..Default::default()
        });
        let environment = environment(operations.clone());
        assert_activity(
            &environment,
            ArchiveActivities::prepare_archive_manifest,
            command.clone(),
            operations.expected(request.manifest.clone()),
        )
        .await;
        assert_activity(
            &environment,
            ArchiveActivities::archive_import_page,
            request.clone(),
            operations.expected(vec![fixtures::import_entry(&request, 0)]),
        )
        .await;
        assert_eq!(
            operations.received.lock().unwrap()[0].1,
            serde_json::to_value(&command).unwrap()
        );
    }
}
