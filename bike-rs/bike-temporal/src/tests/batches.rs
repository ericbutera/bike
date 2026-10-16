use super::*;
use crate::workflows::{
    ActivityArchiveImport, ReprocessArchiveFitActivityImports, ReprocessUserActivityImports,
};
use bike_core::{bike_tasks::*, test_support as fixtures};
use temporalio_common::protos::coresdk::workflow_commands::workflow_command;

pub(super) fn page<T>(request: &PageRequest, factory: fn(&PageRequest, u64) -> T) -> Vec<T> {
    (request.offset..request.offset + request.expected_entries() as u64)
        .map(|index| factory(request, index))
        .collect()
}

pub(super) fn children<I: Serialize>(
    mock: &mut WorkflowMock,
    name: &str,
    inputs: &[I],
    result: &impl Serialize,
) {
    for input in inputs {
        let seq = mock.start_child(name, input);
        // A started child must be awaited before the page may advance.
        mock.assert_no_commands();
        mock.complete_child(seq, result);
    }
}

fn reprocess_scope() -> ManifestScope {
    ManifestScope::Reprocessing {
        user_id: 7,
        selection: ReprocessScope::UserImports,
    }
}

#[test]
fn reprocessing_prepares_one_snapshot_and_awaits_more_than_one_page_of_shared_children() {
    let input = fixtures::batch_input();
    let request = fixtures::page_request(reprocess_scope(), 18, 0);
    let mut mock = WorkflowMock::new::<ReprocessUserActivityImports>(input.clone());
    mock.activity(
        "prepare_reprocess_imports",
        &ReprocessSelection {
            command: input.command,
            scope: ReprocessScope::UserImports,
        },
        &request.manifest,
    );
    let first = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &first);
    children(
        &mut mock,
        "reprocess_activity_import",
        &first,
        &fixtures::import_result(),
    );
    let last = PageRequest {
        offset: 16,
        ..request
    };
    let entries = page(&last, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &last, &entries);
    children(
        &mut mock,
        "reprocess_activity_import",
        &entries,
        &fixtures::import_result(),
    );
    assert_eq!(
        mock.result::<BatchResult>(),
        BatchResult {
            command_id: "fixture-batch".into(),
            completed: 18
        }
    );
}

#[test]
fn archive_fit_reprocessing_uses_its_own_selection_and_the_same_reprocess_child() {
    let input = fixtures::batch_input();
    let scope = ManifestScope::Reprocessing {
        user_id: 7,
        selection: ReprocessScope::ArchiveFits,
    };
    let request = fixtures::page_request(scope, 1, 0);
    let mut mock = WorkflowMock::new::<ReprocessArchiveFitActivityImports>(input.clone());
    mock.activity(
        "prepare_reprocess_imports",
        &ReprocessSelection {
            command: input.command,
            scope: ReprocessScope::ArchiveFits,
        },
        &request.manifest,
    );
    let entries = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &entries);
    children(
        &mut mock,
        "reprocess_activity_import",
        &entries,
        &fixtures::import_result(),
    );
    assert_eq!(mock.result::<BatchResult>().completed, 1);
}

#[test]
fn worker_restart_while_a_child_runs_keeps_the_same_child_and_page_progress() {
    let request = fixtures::page_request(reprocess_scope(), 2, 0);
    let input = fixtures::prepared_batch(&request);
    let mut mock = WorkflowMock::new::<ReprocessUserActivityImports>(input.clone());
    let entries = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &entries);
    let first = mock.start_child("reprocess_activity_import", &entries[0]);
    let mut mock = mock.restart::<ReprocessUserActivityImports>(input);
    mock.complete_child(first, &fixtures::import_result());
    children(
        &mut mock,
        "reprocess_activity_import",
        &entries[1..],
        &fixtures::import_result(),
    );
    assert_eq!(mock.result::<BatchResult>().completed, 2);
}

#[test]
fn continuation_carries_only_settled_progress_and_reuses_the_retained_manifest() {
    let request = fixtures::page_request(reprocess_scope(), 18, 0);
    let input = fixtures::prepared_batch(&request);
    let mut mock = WorkflowMock::new::<ReprocessUserActivityImports>(input);
    let entries = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &entries);
    mock.suggest_continue_as_new();
    children(
        &mut mock,
        "reprocess_activity_import",
        &entries,
        &fixtures::import_result(),
    );
    let continued: BatchInput = mock.continued_input();
    assert_eq!(continued.progress.settled, 16);
    assert_eq!(
        continued.progress.manifest.as_ref(),
        Some(&request.manifest)
    );
    let mut resumed = WorkflowMock::new::<ReprocessUserActivityImports>(continued);
    let last = PageRequest {
        offset: 16,
        ..request
    };
    let entries = page(&last, fixtures::reprocess_entry);
    resumed.activity("reprocess_import_page", &last, &entries);
    children(
        &mut resumed,
        "reprocess_activity_import",
        &entries,
        &fixtures::import_result(),
    );
    assert_eq!(resumed.result::<BatchResult>().completed, 18);
}

#[test]
fn failed_children_and_failed_child_starts_never_report_batch_success() {
    let request = fixtures::page_request(reprocess_scope(), 1, 0);
    for fail_start in [false, true] {
        let mut mock =
            WorkflowMock::new::<ReprocessUserActivityImports>(fixtures::prepared_batch(&request));
        let entries = page(&request, fixtures::reprocess_entry);
        mock.activity("reprocess_import_page", &request, &entries);
        if fail_start {
            mock.fail_child_start("reprocess_activity_import", &entries[0]);
        } else {
            let seq = mock.start_child("reprocess_activity_import", &entries[0]);
            mock.fail_child(seq);
        }
        mock.assert_failed();
    }
}

#[test]
fn empty_manifest_finishes_without_fetching_pages_or_starting_children() {
    let request = fixtures::page_request(reprocess_scope(), 0, 0);
    let mock =
        WorkflowMock::new::<ReprocessUserActivityImports>(fixtures::prepared_batch(&request));
    assert_eq!(mock.result::<BatchResult>().completed, 0);
}

#[test]
fn truncated_duplicate_and_foreign_pages_fail_before_any_child_starts() {
    let request = fixtures::page_request(reprocess_scope(), 2, 0);
    let entries = page(&request, fixtures::reprocess_entry);
    let mut foreign = entries.clone();
    foreign[1].source.user_id = 8;
    let mut foreign_command = entries.clone();
    foreign_command[1].source.command_id = "another-batch:entry-1".into();
    for invalid in [
        vec![entries[0].clone()],
        vec![entries[0].clone(); 2],
        foreign,
        foreign_command,
    ] {
        let mut mock =
            WorkflowMock::new::<ReprocessUserActivityImports>(fixtures::prepared_batch(&request));
        mock.activity("reprocess_import_page", &request, &invalid);
        mock.assert_failed();
    }
}

#[test]
fn continued_input_cannot_substitute_a_different_manifest_selection() {
    let request = fixtures::page_request(reprocess_scope(), 1, 0);
    let mock =
        WorkflowMock::new::<ReprocessArchiveFitActivityImports>(fixtures::prepared_batch(&request));
    mock.assert_failure_message("selection");
}

#[test]
fn native_child_options_keep_stable_identity_priority_and_cancellation() {
    let request = fixtures::page_request(reprocess_scope(), 1, 0);
    let mut mock =
        WorkflowMock::new::<ReprocessUserActivityImports>(fixtures::prepared_batch(&request));
    let entries = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &entries);
    let workflow_command::Variant::StartChildWorkflowExecution(child) = mock.next_command() else {
        panic!("Expected native child workflow")
    };
    assert_eq!(child.workflow_type, "reprocess_activity_import");
    assert!(child.retry_policy.is_none());
    assert_eq!(
        child.workflow_id,
        "bike-task-reprocess_activity_import-user-7-fixture-batch:entry-0"
    );
    assert_eq!(child.task_queue, crate::TASK_QUEUE);
    assert_eq!(child.priority.unwrap().priority_key, 5);
    assert_eq!(
        child.cancellation_type,
        temporalio_common::protos::coresdk::child_workflow::ChildWorkflowCancellationType::WaitCancellationCompleted
            as i32
    );
}

#[test]
fn archive_import_retains_one_manifest_and_reuses_the_process_import_workflow() {
    let input = fixtures::archive_input();
    let request = fixtures::page_request(
        ManifestScope::Archive {
            user_id: 7,
            archive_job_id: 9,
        },
        2,
        0,
    );
    let mut mock = WorkflowMock::new::<ActivityArchiveImport>(input.clone());
    mock.activity(
        "prepare_archive_manifest",
        &ArchiveCommand {
            batch: input.batch.command,
            archive_job_id: input.archive_job_id,
            source: input.source,
        },
        &request.manifest,
    );
    let entries = page(&request, fixtures::import_entry);
    mock.activity("archive_import_page", &request, &entries);
    children(
        &mut mock,
        "process_activity_import",
        &entries,
        &fixtures::import_result(),
    );
    assert_eq!(mock.result::<BatchResult>().completed, 2);
}

#[test]
fn parent_cancellation_waits_for_the_child_and_remains_canceled() {
    let request = fixtures::page_request(reprocess_scope(), 1, 0);
    let mut mock =
        WorkflowMock::new::<ReprocessUserActivityImports>(fixtures::prepared_batch(&request));
    let entries = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &entries);
    let seq = mock.start_child("reprocess_activity_import", &entries[0]);
    mock.cancel_child(seq);
    mock.assert_cancelled();
}

#[test]
fn successful_child_completion_after_parent_cancellation_cannot_finish_the_batch() {
    let request = fixtures::page_request(reprocess_scope(), 1, 0);
    let mut mock =
        WorkflowMock::new::<ReprocessUserActivityImports>(fixtures::prepared_batch(&request));
    let entries = page(&request, fixtures::reprocess_entry);
    mock.activity("reprocess_import_page", &request, &entries);
    let seq = mock.start_child("reprocess_activity_import", &entries[0]);
    mock.cancel();
    assert!(matches!(
        mock.next_command(),
        workflow_command::Variant::CancelChildWorkflowExecution(_)
    ));
    mock.complete_child(seq, &fixtures::import_result());
    mock.assert_cancelled();
}

#[test]
fn archive_manifest_preparation_uses_native_streaming_timeouts_and_heartbeats() {
    let mut mock = WorkflowMock::new::<ActivityArchiveImport>(fixtures::archive_input());
    let workflow_command::Variant::ScheduleActivity(activity) = mock.next_command() else {
        panic!("Expected manifest preparation Activity")
    };
    assert_eq!(activity.activity_type, "prepare_archive_manifest");
    assert_eq!(
        Duration::try_from(activity.start_to_close_timeout.unwrap()).unwrap(),
        policy::MANIFEST_EXECUTION_TIMEOUT
    );
    assert_eq!(
        Duration::try_from(activity.heartbeat_timeout.unwrap()).unwrap(),
        policy::ACTIVITY_HEARTBEAT_TIMEOUT
    );
    assert_eq!(
        activity.retry_policy.unwrap(),
        policy::activity_retry_policy().into_raw()
    );
    assert!(activity.do_not_eagerly_execute);
}

#[test]
fn archive_runs_bound_history_and_continue_without_repreparing_or_double_counting() {
    let request = fixtures::page_request(
        ManifestScope::Archive {
            user_id: 7,
            archive_job_id: 9,
        },
        273,
        0,
    );
    let mut input = fixtures::archive_input();
    input.batch = fixtures::prepared_batch(&request);
    let source = input.source.clone();
    let mut mock = WorkflowMock::new::<ActivityArchiveImport>(input.clone());
    for offset in (0..256).step_by(PAGE_SIZE) {
        let page_request = PageRequest {
            offset,
            ..request.clone()
        };
        let entries = page(&page_request, fixtures::import_entry);
        mock.activity("archive_import_page", &page_request, &entries);
        children(
            &mut mock,
            "process_activity_import",
            &entries,
            &fixtures::import_result(),
        );
    }
    let continued: ArchiveInput = mock.continued_input();
    assert_eq!(continued.source, source);
    assert_eq!(continued.batch.progress.settled, 256);
    let mut resumed = WorkflowMock::new::<ActivityArchiveImport>(continued);
    for offset in (256..273).step_by(PAGE_SIZE) {
        let page_request = PageRequest {
            offset,
            ..request.clone()
        };
        let entries = page(&page_request, fixtures::import_entry);
        resumed.activity("archive_import_page", &page_request, &entries);
        children(
            &mut resumed,
            "process_activity_import",
            &entries,
            &fixtures::import_result(),
        );
    }
    assert_eq!(resumed.result::<BatchResult>().completed, 273);
}
