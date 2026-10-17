use super::*;
use bike_core::test_support as fixtures;

pub(super) fn complete_derived_tasks(mock: &mut WorkflowMock, imported: &ActivityImportResult) {
    let task = imported.activity();
    mock.activity("rebuild_activity_segments", &task, &vec![18_i32, 19]);
    for segment_id in [18, 19] {
        let segment = SegmentChange {
            segment_id,
            priority: imported.priority,
        };
        let child = mock.start_child("rebuild_segment_analytics", &segment);
        mock.assert_pending();
        let mut segments = WorkflowMock::new::<RebuildSegmentAnalytics>(segment.clone());
        segments.activity("rebuild_segment_analytics", &segment, &());
        mock.complete_child(child, &segments.result::<()>());
    }
    mock.activity("rebuild_activity_achievements", &task, &());
    let child = mock.start_child("prepare_heatmap", &task);
    mock.assert_pending();
    let mut heatmap = WorkflowMock::new::<PrepareHeatmap>(task.clone());
    heatmap.activity("prepare_heatmap", &task, &());
    mock.complete_child(child, &heatmap.result::<()>());
    mock.activity("rebuild_activity_training", &task, &());
    let fitness = FitnessChange {
        user_id: imported.user_id,
        priority: imported.priority,
    };
    let child = mock.start_child("rebuild_fitness_freshness", &fitness);
    mock.assert_pending();
    let mut rebuild = WorkflowMock::new::<RebuildFitnessFreshness>(fitness.clone());
    rebuild.activity("rebuild_fitness_freshness", &fitness, &());
    mock.complete_child(child, &rebuild.result::<()>());
}

#[test]
fn import_saves_source_then_awaits_segments_achievements_heatmap_and_fitness() {
    let command = fixtures::import_command();
    let result = fixtures::import_result();
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    mock.activity("activity_import", &command, &result);
    complete_derived_tasks(&mut mock, &result);
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

#[test]
fn reprocessing_resumes_after_source_save_and_awaits_real_derived_children() {
    let input = fixtures::reprocess_input();
    let result = fixtures::import_result();
    let mut mock = WorkflowMock::new::<ReprocessActivityImport>(input.clone());
    mock.activity("activity_import", &input.clone().into_command(), &result);
    let mut mock = mock.restart::<ReprocessActivityImport>(input);
    complete_derived_tasks(&mut mock, &result);
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

#[test]
fn source_failure_stops_the_workflow_before_derived_steps() {
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(fixtures::import_command().source);
    mock.fail_activity("activity_import", "source failed");
    mock.assert_failure_message("source failed");
}

#[test]
fn segment_child_failure_fails_the_parent_before_achievements() {
    let command = fixtures::import_command();
    let result = fixtures::import_result();
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    mock.activity("activity_import", &command, &result);
    mock.activity("rebuild_activity_segments", &result.activity(), &vec![18_i32]);
    let child = mock.start_child(
        "rebuild_segment_analytics",
        &SegmentChange {
            segment_id: 18,
            priority: result.priority,
        },
    );
    mock.fail_child(child);
    mock.assert_failure_message("Fixture child failed");
}

#[test]
fn empty_segments_still_rebuild_activity_achievements_and_other_outputs() {
    let command = fixtures::import_command();
    let result = fixtures::import_result();
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(command.source.clone());
    mock.activity("activity_import", &command, &result);
    mock.activity("rebuild_activity_segments", &result.activity(), &Vec::<i32>::new());
    mock.activity("rebuild_activity_achievements", &result.activity(), &());
    let child = mock.start_child("prepare_heatmap", &result.activity());
    mock.complete_child(child, &());
    mock.activity("rebuild_activity_training", &result.activity(), &());
    let child = mock.start_child(
        "rebuild_fitness_freshness",
        &FitnessChange {
            user_id: result.user_id,
            priority: result.priority,
        },
    );
    mock.complete_child(child, &());
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

#[test]
fn derived_workflows_execute_one_named_activity_and_complete() {
    let task = fixtures::activity_task();
    let mut heatmap = WorkflowMock::new::<PrepareHeatmap>(task.clone());
    heatmap.activity("prepare_heatmap", &task, &());
    heatmap.result::<()>();
    let task = fixtures::fitness_change();
    let mut fitness = WorkflowMock::new::<RebuildFitnessFreshness>(task.clone());
    fitness.activity("rebuild_fitness_freshness", &task, &());
    fitness.result::<()>();
    let task = fixtures::segment_change();
    let mut segments = WorkflowMock::new::<RebuildSegmentAnalytics>(task.clone());
    segments.activity("rebuild_segment_analytics", &task, &());
    segments.result::<()>();
}

#[test]
fn cancellation_before_processing_starts_prevents_work() {
    WorkflowMock::cancelled_at_start::<ProcessActivityImport>(fixtures::import_command().source).assert_cancelled();
}

#[test]
fn cancellation_lets_the_running_source_settle_and_stops_later_steps() {
    use temporalio_common::protos::coresdk::{
        activity_result,
        workflow_activation::{self, workflow_activation_job::Variant},
        workflow_commands::workflow_command,
    };
    let mut mock = WorkflowMock::new::<ProcessActivityImport>(fixtures::import_command().source);
    let workflow_command::Variant::ScheduleActivity(source) = mock.next_command() else {
        panic!("Expected source Activity");
    };
    mock.cancel();
    assert!(matches!(
        mock.next_command(),
        workflow_command::Variant::RequestCancelActivity(_)
    ));
    mock.activate(Variant::ResolveActivity(workflow_activation::ResolveActivity {
        seq: source.seq,
        result: Some(activity_result::ActivityResolution {
            status: Some(activity_result::activity_resolution::Status::Completed(
                activity_result::Success {
                    result: Some(workflow_test_support::payload(&fixtures::import_result())),
                },
            )),
        }),
        ..Default::default()
    }));
    mock.assert_cancelled();
}

#[test]
fn replay_after_segment_completion_resumes_with_achievements() {
    let input = fixtures::reprocess_input();
    let result = fixtures::import_result();
    let mut mock = WorkflowMock::new::<ReprocessActivityImport>(input.clone());
    mock.activity("activity_import", &input.clone().into_command(), &result);
    mock.activity("rebuild_activity_segments", &result.activity(), &vec![18_i32]);
    let child = mock.start_child(
        "rebuild_segment_analytics",
        &SegmentChange {
            segment_id: 18,
            priority: result.priority,
        },
    );
    mock.complete_child(child, &());
    let mut mock = mock.restart::<ReprocessActivityImport>(input);
    mock.activity("rebuild_activity_achievements", &result.activity(), &());
    let child = mock.start_child("prepare_heatmap", &result.activity());
    mock.complete_child(child, &());
    mock.activity("rebuild_activity_training", &result.activity(), &());
    let child = mock.start_child(
        "rebuild_fitness_freshness",
        &FitnessChange {
            user_id: result.user_id,
            priority: result.priority,
        },
    );
    mock.complete_child(child, &());
    assert_eq!(mock.result::<ActivityImportResult>(), result);
}

fn saved_import_waiting_for_heatmap(reprocessing: bool) -> (WorkflowMock, ActivityImportResult, u32) {
    let command = if reprocessing {
        fixtures::reprocess_input().into_command()
    } else {
        fixtures::import_command()
    };
    let mut mock = if reprocessing {
        WorkflowMock::new::<ReprocessActivityImport>(fixtures::reprocess_input())
    } else {
        WorkflowMock::new::<ProcessActivityImport>(command.source.clone())
    };
    let result = fixtures::import_result();
    mock.activity("activity_import", &command, &result);
    mock.activity("rebuild_activity_segments", &result.activity(), &Vec::<i32>::new());
    mock.activity("rebuild_activity_achievements", &result.activity(), &());
    let child = mock.start_child("prepare_heatmap", &result.activity());
    mock.assert_pending();
    (mock, result, child)
}

#[test]
fn failed_heatmap_stops_processing_and_reprocessing_before_training() {
    for reprocessing in [false, true] {
        let (mut mock, _, child) = saved_import_waiting_for_heatmap(reprocessing);
        mock.fail_child(child);
        mock.assert_failure_message("Fixture child failed");
    }
}

#[test]
fn failed_fitness_prevents_processing_and_reprocessing_from_reporting_success() {
    for reprocessing in [false, true] {
        let (mut mock, result, child) = saved_import_waiting_for_heatmap(reprocessing);
        mock.complete_child(child, &());
        mock.activity("rebuild_activity_training", &result.activity(), &());
        let child = mock.start_child(
            "rebuild_fitness_freshness",
            &FitnessChange {
                user_id: result.user_id,
                priority: result.priority,
            },
        );
        mock.assert_pending();
        mock.fail_child(child);
        mock.assert_failure_message("Fixture child failed");
    }
}

#[test]
fn cancelled_child_cancels_processing_and_reprocessing_without_starting_later_work() {
    for reprocessing in [false, true] {
        let (mut mock, _, child) = saved_import_waiting_for_heatmap(reprocessing);
        mock.cancel_child(child);
        mock.assert_cancelled();
    }
}
