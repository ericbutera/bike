use super::*;
use bike_core::bike_tasks::{ActivityImportResult, ImportTarget};
use temporalio_workflow::common::WorkflowDefinition;

fn handoff(mock: &mut WorkflowMock, result: &ActivityImportResult) {
    mock.activity("schedule_fitness_freshness", &result.fitness, &());
    if !result.segments.is_empty() {
        mock.activity("schedule_segment_analytics", &result.segments, &());
    }
    if let Some(heatmap) = &result.heatmap {
        mock.activity("schedule_heatmap", heatmap, &());
    }
}

#[test]
fn new_import_uses_shared_activity_then_hands_off_without_waiting_for_outputs() {
    let command = import_command();
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    let result = import_result();
    mock.activity("activity_import", &command, &result);
    handoff(&mut mock, &result);
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

#[test]
fn reprocess_uses_the_same_activity_with_an_explicit_existing_target() {
    let mut command = import_command();
    command.target = ImportTarget::ExistingActivity {
        activity_id: 42,
        expected_revision: 3,
    };
    let input = ReprocessInput {
        source: command.source.clone(),
        activity_id: 42,
        expected_revision: 3,
    };
    let mut mock = WorkflowMock::new::<ReprocessActivityImport>(input.clone());
    let result = import_result();
    mock.activity("activity_import", &command, &result);
    let mut mock = mock.restart::<ReprocessActivityImport>(input);
    handoff(&mut mock, &result);
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

#[test]
fn import_failure_does_not_schedule_derived_work() {
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(import_command().source);
    mock.fail_activity("activity_import", "source failed");
    mock.assert_failure_message("source failed");
}

#[test]
fn handoff_failure_does_not_report_an_accepted_import_pipeline() {
    let command = import_command();
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    mock.activity("activity_import", &command, &import_result());
    mock.fail_activity("schedule_fitness_freshness", "handoff failed");
    mock.assert_failure_message("handoff failed");
}

#[test]
fn optional_heatmap_and_empty_segment_set_do_not_block_import_completion() {
    let command = import_command();
    let result = ActivityImportResult {
        segments: vec![],
        heatmap: None,
        ..import_result()
    };
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    mock.activity("activity_import", &command, &result);
    handoff(&mut mock, &result);
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

#[test]
fn workflow_types_match_the_existing_conversion_plan_names() {
    use crate::workflows::*;
    assert_eq!(ProcessActivityImport::run.name(), "process_activity_import");
    assert_eq!(
        ReprocessActivityImport::run.name(),
        "reprocess_activity_import"
    );
    assert_eq!(
        RebuildFitnessFreshness::run.name(),
        "rebuild_fitness_freshness"
    );
    assert_eq!(
        RebuildSegmentAnalytics::run.name(),
        "rebuild_segment_analytics"
    );
    assert_eq!(PrepareHeatmap::run.name(), "prepare_heatmap");
    for (actual, expected) in [
        (
            RegenerateActivitySegments::run.name(),
            "regenerate_activity_segments",
        ),
        (
            RegenerateSegmentEfforts::run.name(),
            "regenerate_segment_efforts",
        ),
        (
            BackfillUserXcTraining::run.name(),
            "backfill_user_xc_training",
        ),
        (
            ReprocessUserActivityImports::run.name(),
            "reprocess_user_activity_imports",
        ),
        (
            ReprocessArchiveFitActivityImports::run.name(),
            "reprocess_archive_fit_activity_imports",
        ),
        (
            RegenerateUserSegments::run.name(),
            "regenerate_user_segments",
        ),
        (ActivityArchiveImport::run.name(), "activity_archive_import"),
    ] {
        assert_eq!(actual, expected);
    }
}

#[test]
fn invalid_reprocess_revision_fails_before_any_side_effects() {
    let input = ReprocessInput {
        source: import_command().source,
        activity_id: 42,
        expected_revision: 0,
    };
    let mock = WorkflowMock::new::<ReprocessActivityImport>(input);
    mock.assert_failure_message("positive committed revision");
}

#[test]
fn late_successful_import_after_cancellation_does_not_handoff_derived_work() {
    let command = import_command();
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    let seq = scheduled(&mut mock, "activity_import", &command, 1, "user-7");
    mock.cancel();
    assert!(matches!(
        mock.next_command(),
        workflow_command::Variant::RequestCancelActivity(_)
    ));
    mock.activate(Variant::ResolveActivity(
        workflow_activation::ResolveActivity {
            seq,
            result: Some(activity_result::ActivityResolution {
                status: Some(activity_result::activity_resolution::Status::Completed(
                    activity_result::Success {
                        result: Some(payload(&import_result())),
                    },
                )),
            }),
            ..Default::default()
        },
    ));
    mock.assert_cancelled();
}
