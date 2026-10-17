use crate::workflows::{
    DebouncePolicy, PrepareHeatmap, ProcessActivityImport, RebuildFitnessFreshness, RebuildInput,
    RebuildSegmentAnalytics, ReprocessActivityImport, ReprocessInput,
};
use bike_core::{
    bike_tasks::{FitnessChange, Priority, Publication},
    platform::temporal as policy,
    test_support::{fitness_change, heatmap_change, import_command, import_result, segment_change},
};
use serde::Serialize;
use std::time::{Duration, UNIX_EPOCH};
use temporalio_common::protos::coresdk::{
    activity_result,
    workflow_activation::{self, workflow_activation_job::Variant},
    workflow_commands::workflow_command,
};
use workflow_test_support::payload;
type WorkflowMock = workflow_test_support::WorkflowMock;
mod adapters;
mod batches;
pub(crate) mod client;
mod command_identity;
pub(crate) mod fixtures;
mod handlers;
mod import_operations;
mod imports;
mod segments;
mod submission;
mod training;

fn input() -> RebuildInput<FitnessChange> {
    RebuildInput::new(fitness_change(), DebouncePolicy::default())
}
fn change(revision: u64, priority: Priority) -> FitnessChange {
    FitnessChange {
        revision,
        priority,
        ..fitness_change()
    }
}
fn signal(mock: &mut WorkflowMock, change: &impl Serialize, second: u64) {
    mock.activate_at(
        Variant::SignalWorkflow(workflow_activation::SignalWorkflow {
            signal_name: "changed".into(),
            input: vec![payload(change)],
            ..Default::default()
        }),
        UNIX_EPOCH + Duration::from_secs(second),
    );
}
fn timer(mock: &mut WorkflowMock, seconds: i64) -> u32 {
    let workflow_command::Variant::StartTimer(timer) = mock.next_command() else {
        panic!("Expected native timer")
    };
    assert_eq!(timer.start_to_fire_timeout.unwrap().seconds, seconds);
    timer.seq
}
fn canceled_timer(mock: &mut WorkflowMock, seq: u32) {
    let workflow_command::Variant::CancelTimer(timer) = mock.next_command() else {
        panic!("Expected timer cancellation")
    };
    assert_eq!(timer.seq, seq);
}
fn fire(mock: &mut WorkflowMock, seq: u32, second: u64) {
    mock.activate_at(
        Variant::FireTimer(workflow_activation::FireTimer { seq }),
        UNIX_EPOCH + Duration::from_secs(second),
    );
}
fn scheduled(mock: &mut WorkflowMock, name: &str, expected: &impl Serialize, priority: i32, fairness: &str) -> u32 {
    let workflow_command::Variant::ScheduleActivity(activity) = mock.next_command() else {
        panic!("Expected native Activity")
    };
    assert_eq!(activity.activity_type, name);
    assert_eq!(activity.task_queue, crate::TASK_QUEUE);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&activity.arguments[0].data).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let native = activity.priority.unwrap();
    assert_eq!(native.priority_key, priority);
    assert_eq!(native.fairness_key, fairness);
    assert!(activity.do_not_eagerly_execute);
    assert_eq!(
        activity.retry_policy.unwrap(),
        policy::activity_retry_policy().into_raw()
    );
    assert_eq!(
        Duration::try_from(activity.start_to_close_timeout.unwrap()).unwrap(),
        policy::ACTIVITY_EXECUTION_TIMEOUT
    );
    activity.seq
}
fn complete(mock: &mut WorkflowMock, seq: u32, publication: Publication) {
    mock.activate(Variant::ResolveActivity(workflow_activation::ResolveActivity {
        seq,
        result: Some(activity_result::ActivityResolution {
            status: Some(activity_result::activity_resolution::Status::Completed(
                activity_result::Success {
                    result: Some(payload(&publication)),
                },
            )),
        }),
        ..Default::default()
    }));
}

#[test]
fn five_quick_requests_for_the_same_user_rebuild_once_at_the_latest_revision() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    for revision in 1..=5 {
        signal(&mut mock, &change(revision, Priority::Background), revision - 1);
        if revision > 1 {
            canceled_timer(&mut mock, revision as u32 - 1);
        }
        timer(&mut mock, 60);
    }
    fire(&mut mock, 5, 64);
    let seq = scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(5, Priority::Background),
        5,
        "user-7",
    );
    complete(&mut mock, seq, Publication::Published);
    mock.assert_no_commands();
}

#[test]
fn new_activity_advances_a_background_flush_and_later_delete_does_not_delay_it() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::Background), 0);
    let first = timer(&mut mock, 60);
    signal(&mut mock, &change(2, Priority::NewActivity), 1);
    canceled_timer(&mut mock, first);
    let promoted = timer(&mut mock, 2);
    signal(&mut mock, &change(3, Priority::Background), 2);
    canceled_timer(&mut mock, promoted);
    let last = timer(&mut mock, 1);
    fire(&mut mock, last, 3);
    scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(3, Priority::NewActivity),
        1,
        "user-7",
    );
}

#[test]
fn native_replay_preserves_active_work_and_one_merged_follow_up() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::NewActivity), 0);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 2);
    let first = scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(1, Priority::NewActivity),
        1,
        "user-7",
    );
    for revision in 2..=6 {
        signal(&mut mock, &change(revision, Priority::Background), revision + 1);
    }
    mock.assert_no_commands();
    let mut mock = mock.restart::<RebuildFitnessFreshness>(input());
    complete(&mut mock, first, Publication::Published);
    let wait = timer(&mut mock, 60);
    fire(&mut mock, wait, 67);
    let second = scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(6, Priority::Background),
        5,
        "user-7",
    );
    complete(&mut mock, second, Publication::Published);
    mock.assert_no_commands();
}

#[test]
fn continue_as_new_preserves_pending_requests_and_timer_deadlines() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::Background), 0);
    let wait = timer(&mut mock, 60);
    mock.suggest_continue_as_new();
    fire(&mut mock, wait, 60);
    let continued: RebuildInput<FitnessChange> = mock.continued_input();
    let mut resumed = WorkflowMock::at_time::<RebuildFitnessFreshness>(continued, UNIX_EPOCH + Duration::from_secs(60));
    scheduled(
        &mut resumed,
        "rebuild_fitness_freshness",
        &change(1, Priority::Background),
        5,
        "user-7",
    );
}

#[test]
fn segment_requests_from_multiple_parents_share_one_segment_output() {
    let original = segment_change();
    let mut mock =
        WorkflowMock::new::<RebuildSegmentAnalytics>(RebuildInput::new(original.clone(), DebouncePolicy::default()));
    let mut wait = None;
    for revision in 1..=5 {
        signal(
            &mut mock,
            &bike_core::bike_tasks::SegmentChange {
                revision,
                ..original.clone()
            },
            revision - 1,
        );
        if let Some(previous) = wait {
            canceled_timer(&mut mock, previous);
        }
        wait = Some(timer(&mut mock, 60));
    }
    fire(&mut mock, wait.unwrap(), 64);
    let expected = bike_core::bike_tasks::SegmentChange {
        revision: 5,
        ..original
    };
    let seq = scheduled(&mut mock, "rebuild_segment_analytics", &expected, 5, "segment-18");
    complete(&mut mock, seq, Publication::Published);
    mock.assert_no_commands();
}

#[test]
fn heatmap_workflow_prepares_the_latest_generation() {
    let original = heatmap_change();
    let mut mock = WorkflowMock::new::<PrepareHeatmap>(RebuildInput::new(original.clone(), DebouncePolicy::default()));
    let request = bike_core::bike_tasks::HeatmapChange {
        generation: 3,
        priority: Priority::NewActivity,
        ..original
    };
    signal(&mut mock, &request, 0);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 2);
    let seq = scheduled(&mut mock, "prepare_heatmap", &request, 1, "user-7");
    complete(&mut mock, seq, Publication::Published);
    mock.assert_no_commands();
}

#[test]
fn terminal_publication_failure_remains_visible() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::Background), 0);
    let wait = timer(&mut mock, 60);
    fire(&mut mock, wait, 60);
    mock.fail_activity("rebuild_fitness_freshness", "publication failed");
    mock.assert_failure_message("publication failed");
}

#[test]
fn cancellation_uses_native_activity_cancellation() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::NewActivity), 0);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 2);
    mock.cancel_activity("rebuild_fitness_freshness");
    mock.assert_cancelled();
}

#[test]
fn cancellation_while_debouncing_cancels_the_timer() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::Background), 0);
    let wait = timer(&mut mock, 60);
    mock.cancel();
    canceled_timer(&mut mock, wait);
    mock.assert_cancelled();
}

#[test]
fn late_notification_is_rebuilt_even_after_a_higher_revision_published() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(5, Priority::NewActivity), 0);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 2);
    let first = scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(5, Priority::NewActivity),
        1,
        "user-7",
    );
    complete(&mut mock, first, Publication::Published);
    mock.assert_no_commands();
    signal(&mut mock, &change(1, Priority::NewActivity), 3);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 5);
    scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(1, Priority::NewActivity),
        1,
        "user-7",
    );
}

#[test]
fn superseded_output_waits_for_a_new_revision_without_spinning() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(&mut mock, &change(1, Priority::NewActivity), 0);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 2);
    let first = scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(1, Priority::NewActivity),
        1,
        "user-7",
    );
    complete(&mut mock, first, Publication::Superseded);
    mock.assert_no_commands();
    signal(&mut mock, &change(2, Priority::Background), 3);
    scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(2, Priority::NewActivity),
        1,
        "user-7",
    );
}

#[test]
fn duplicate_pending_request_does_not_extend_the_existing_deadline() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    let request = change(1, Priority::Background);
    signal(&mut mock, &request, 0);
    let first = timer(&mut mock, 60);
    signal(&mut mock, &request, 20);
    canceled_timer(&mut mock, first);
    let remaining = timer(&mut mock, 40);
    fire(&mut mock, remaining, 60);
    scheduled(&mut mock, "rebuild_fitness_freshness", &request, 5, "user-7");
}

#[test]
fn out_of_order_fitness_requests_keep_the_earliest_dirty_date() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    let latest = change(5, Priority::NewActivity);
    signal(&mut mock, &latest, 0);
    let first = timer(&mut mock, 2);
    let earlier = FitnessChange {
        revision: 4,
        dirty_from: latest.dirty_from - chrono::TimeDelta::days(30),
        ..latest.clone()
    };
    signal(&mut mock, &earlier, 1);
    canceled_timer(&mut mock, first);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 3);
    let expected = FitnessChange {
        dirty_from: earlier.dirty_from,
        ..latest
    };
    scheduled(&mut mock, "rebuild_fitness_freshness", &expected, 1, "user-7");
}

#[test]
fn continuous_requests_flush_at_the_cap_in_both_priority_classes() {
    for (priority, spacing, count, quiet, cap, key) in [
        (Priority::Background, 59, 6, 60, 300, 5),
        (Priority::NewActivity, 1, 10, 2, 10, 1),
    ] {
        let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
        let mut previous = None;
        for revision in 1..=count {
            let at = (revision - 1) * spacing;
            signal(&mut mock, &change(revision, priority), at);
            if let Some(wait) = previous {
                canceled_timer(&mut mock, wait);
            }
            previous = Some(timer(&mut mock, quiet.min(cap - at) as i64));
        }
        fire(&mut mock, previous.unwrap(), cap);
        scheduled(
            &mut mock,
            "rebuild_fitness_freshness",
            &change(count, priority),
            key,
            "user-7",
        );
    }
}

#[test]
fn invalid_windows_and_cross_resource_signals_fail_before_scheduling_work() {
    let invalid = DebouncePolicy {
        background_quiet: chrono::TimeDelta::zero(),
        ..DebouncePolicy::default()
    };
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(RebuildInput::new(fitness_change(), invalid));
    signal(&mut mock, &fitness_change(), 0);
    mock.assert_failure_message("0 < quiet <= cap");
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    signal(
        &mut mock,
        &FitnessChange {
            user_id: 8,
            ..fitness_change()
        },
        0,
    );
    mock.assert_failure_message("this resource");
}
