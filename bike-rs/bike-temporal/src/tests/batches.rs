use super::*;
use bike_core::test_support as fixtures;

fn page() -> UserPage {
    UserPage {
        user_id: fixtures::activity_task().user_id,
        after_id: 0,
    }
}
fn cycling(page: UserPage) -> ActivityPage {
    ActivityPage {
        page,
        selection: ActivitySelection::Cycling,
    }
}
fn imports(page: UserPage, archive_fits_only: bool) -> ActivityPage {
    ActivityPage {
        page,
        selection: ActivitySelection::Imports { archive_fits_only },
    }
}
fn background_activity(activity_id: i32) -> ActivityTask {
    ActivityTask {
        activity_id,
        priority: Priority::Background,
        ..fixtures::activity_task()
    }
}

#[test]
fn regenerate_activity_awaits_segment_results_before_achievements() {
    let task = background_activity(42);
    let mut mock = WorkflowMock::new::<RegenerateActivitySegments>(task.clone());
    mock.activity("rebuild_activity_segments", &task, &vec![18_i32]);
    let child = mock.start_child(
        "rebuild_segment_analytics",
        &SegmentChange {
            segment_id: 18,
            priority: Priority::Background,
        },
    );
    mock.complete_child(child, &());
    mock.activity("rebuild_activity_achievements", &task, &());
    mock.result::<()>();
}

#[test]
fn segment_regeneration_calls_its_domain_operation() {
    let mut mock = WorkflowMock::new::<RegenerateSegmentEfforts>(SegmentChange {
        segment_id: 18,
        priority: Priority::Background,
    });
    mock.activity("regenerate_segment_efforts", &18_i32, &());
    mock.result::<()>();
}

#[test]
fn user_segments_pages_continue_after_awaited_children() {
    let mut input = page();
    for batch in [0..16, 16..32] {
        let tasks: Vec<_> = batch.map(|id| background_activity(id + 1)).collect();
        let mut mock = WorkflowMock::new::<RegenerateUserSegments>(input.clone());
        mock.activity("activity_page", &cycling(input.clone()), &tasks);
        for task in &tasks {
            let child = mock.start_child("regenerate_activity_segments", task);
            mock.complete_child(child, &());
        }
        input = mock.continued_input::<UserPage>();
        assert_eq!(input.after_id, tasks.last().unwrap().activity_id);
    }
    let mut mock = WorkflowMock::new::<RegenerateUserSegments>(input.clone());
    mock.activity("activity_page", &cycling(input), &Vec::<ActivityTask>::new());
    mock.result::<()>();
}

#[test]
fn user_imports_load_owned_inputs_and_await_reprocessing() {
    let task = background_activity(fixtures::reprocess_input().activity_id);
    let input = fixtures::reprocess_input();
    let mut mock = WorkflowMock::new::<ReprocessUserActivityImports>(page());
    mock.activity("activity_page", &imports(page(), false), &vec![task.clone()]);
    mock.activity("reprocess_input", &task, &input);
    let child = mock.start_child("reprocess_activity_import", &input);
    mock.complete_child(child, &fixtures::import_result());
    mock.result::<()>();
}

#[test]
fn archive_fit_reprocessing_keeps_its_scope() {
    let mut mock = WorkflowMock::new::<ReprocessArchiveFitActivityImports>(page());
    mock.activity("activity_page", &imports(page(), true), &Vec::<ActivityTask>::new());
    mock.result::<()>();
}

#[test]
fn training_backfill_calculates_each_activity_then_marks_domain_status() {
    let task = background_activity(42);
    let mut mock = WorkflowMock::new::<BackfillUserXcTraining>(page());
    mock.activity("activity_page", &cycling(page()), &vec![task.clone()]);
    mock.activity("rebuild_activity_training", &task, &());
    mock.activity("finish_training_backfill", &page().user_id, &());
    mock.result::<()>();
}

fn archive_cursor() -> ArchiveCursor {
    ArchiveCursor {
        job_id: 7,
        archive_path: "/uploads/archives/7.zip".into(),
        offset: 0,
    }
}

#[test]
fn archive_replay_reuses_extracted_page_then_continues_with_a_small_cursor() {
    let input = ArchiveInput {
        job_id: 7,
        cursor: None,
    };
    let cursor = archive_cursor();
    let source = fixtures::import_command().source;
    let mut mock = WorkflowMock::new::<ActivityArchiveImport>(input.clone());
    mock.activity("prepare_archive", &7_i32, &cursor);
    mock.activity(
        "archive_page",
        &cursor,
        &ArchivePage {
            sources: vec![source.clone()],
            next_offset: 16,
            done: false,
        },
    );
    let mut mock = mock.restart::<ActivityArchiveImport>(input);
    let child = mock.start_child("process_activity_import", &source);
    mock.complete_child(child, &fixtures::import_result());
    let next = mock.continued_input::<ArchiveInput>();
    assert_eq!(next.cursor.as_ref().unwrap().offset, 16);
    let mut mock = WorkflowMock::new::<ActivityArchiveImport>(next.clone());
    mock.activity(
        "archive_page",
        &next.cursor.unwrap(),
        &ArchivePage {
            sources: vec![],
            next_offset: 16,
            done: true,
        },
    );
    mock.activity("finish_archive", &7_i32, &());
    mock.result::<()>();
}

#[test]
fn failed_archive_child_stops_before_advancing_or_marking_complete() {
    let source = fixtures::import_command().source;
    let mut mock = WorkflowMock::new::<ActivityArchiveImport>(ArchiveInput {
        job_id: 7,
        cursor: Some(archive_cursor()),
    });
    mock.activity(
        "archive_page",
        &archive_cursor(),
        &ArchivePage {
            sources: vec![source.clone()],
            next_offset: 1,
            done: true,
        },
    );
    let child = mock.start_child("process_activity_import", &source);
    mock.fail_child(child);
    mock.assert_failure_message("Fixture child failed");
}

#[test]
fn reconciliation_awaits_heatmaps_and_handles_an_empty_page() {
    let task = background_activity(42);
    let mut mock = WorkflowMock::new::<ReconcileHeatmaps>(0);
    mock.activity("pending_heatmaps", &0_i32, &vec![task.clone()]);
    let child = mock.start_child("prepare_heatmap", &task);
    mock.complete_child(child, &());
    mock.result::<()>();
    let mut mock = WorkflowMock::new::<ReconcileHeatmaps>(0);
    mock.activity("pending_heatmaps", &0_i32, &Vec::<ActivityTask>::new());
    mock.result::<()>();
}
