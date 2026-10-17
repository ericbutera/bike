use super::*;

fn drain(mock: &mut WorkflowMock) {
    mock.activate(Variant::SignalWorkflow(workflow_activation::SignalWorkflow {
        signal_name: "drain".into(),
        ..Default::default()
    }));
}

fn publish(mock: &mut WorkflowMock, revision: u64, at: u64) {
    let seq = start_publication(mock, revision, at);
    complete(mock, seq, Publication::Published);
}

fn start_publication(mock: &mut WorkflowMock, revision: u64, at: u64) -> u32 {
    let request = change(revision, Priority::NewActivity);
    signal(mock, &request, at);
    let wait = timer(mock, 2);
    fire(mock, wait, at + 2);
    scheduled(mock, "rebuild_fitness_freshness", &request, 1, "user-7")
}

#[test]
fn new_output_run_finishes_after_publication() {
    let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    publish(&mut mock, 1, 0);
    mock.result::<()>();
}

#[test]
fn a_later_run_accepts_lower_revisions_after_a_previous_run_completed() {
    let mut first = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    publish(&mut first, 5, 0);
    first.result::<()>();
    let mut next = WorkflowMock::new::<RebuildFitnessFreshness>(input());
    publish(&mut next, 1, 3);
    next.result::<()>();
}

#[test]
fn completion_and_signal_in_the_same_activation_preserve_the_follow_up_in_either_order() {
    for signal_first in [false, true] {
        let mut mock = WorkflowMock::new::<RebuildFitnessFreshness>(input());
        let first = start_publication(&mut mock, 5, 0);
        let mut jobs = vec![
            workflow_activation::WorkflowActivationJob {
                variant: Some(completed(first, Publication::Published)),
            },
            workflow_activation::WorkflowActivationJob {
                variant: Some(Variant::SignalWorkflow(workflow_activation::SignalWorkflow {
                    signal_name: "changed".into(),
                    input: vec![payload(&change(1, Priority::NewActivity))],
                    ..Default::default()
                })),
            },
        ];
        if signal_first {
            jobs.reverse();
        }
        mock.apply_activation(workflow_activation::WorkflowActivation {
            timestamp: Some((UNIX_EPOCH + Duration::from_secs(3)).into()),
            jobs,
            ..Default::default()
        });
        let wait = timer(&mut mock, 2);
        fire(&mut mock, wait, 5);
        let next = scheduled(
            &mut mock,
            "rebuild_fitness_freshness",
            &change(1, Priority::NewActivity),
            1,
            "user-7",
        );
        complete(&mut mock, next, Publication::Published);
        mock.result::<()>();
    }
}

#[test]
fn legacy_history_replays_multiple_publications_then_drains_without_new_work() {
    let mut mock = WorkflowMock::with_replay::<RebuildFitnessFreshness>(input(), true);
    publish(&mut mock, 1, 0);
    publish(&mut mock, 2, 3);
    let mut mock = mock.restart::<RebuildFitnessFreshness>(input());
    drain(&mut mock);
    mock.result::<()>();
}

#[test]
fn drain_keeps_pending_work_and_its_existing_quiet_deadline() {
    let mut mock = WorkflowMock::with_replay::<RebuildFitnessFreshness>(input(), true);
    let request = change(1, Priority::Background);
    signal(&mut mock, &request, 0);
    let initial = timer(&mut mock, 60);
    drain(&mut mock);
    canceled_timer(&mut mock, initial);
    let wait = timer(&mut mock, 60);
    fire(&mut mock, wait, 60);
    let seq = scheduled(&mut mock, "rebuild_fitness_freshness", &request, 5, "user-7");
    complete(&mut mock, seq, Publication::Published);
    mock.result::<()>();
}

#[test]
fn drain_waits_for_active_publication_and_all_accepted_follow_up_changes() {
    let mut mock = WorkflowMock::with_replay::<RebuildFitnessFreshness>(input(), true);
    let request = change(5, Priority::NewActivity);
    signal(&mut mock, &request, 0);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 2);
    let first = scheduled(&mut mock, "rebuild_fitness_freshness", &request, 1, "user-7");
    drain(&mut mock);
    signal(&mut mock, &change(1, Priority::NewActivity), 3);
    signal(&mut mock, &change(2, Priority::NewActivity), 4);
    mock.assert_no_commands();
    complete(&mut mock, first, Publication::Published);
    let wait = timer(&mut mock, 2);
    fire(&mut mock, wait, 6);
    let second = scheduled(
        &mut mock,
        "rebuild_fitness_freshness",
        &change(2, Priority::NewActivity),
        1,
        "user-7",
    );
    complete(&mut mock, second, Publication::Published);
    mock.result::<()>();
}

#[test]
fn legacy_idle_segment_and_heatmap_runs_can_drain_without_an_activity() {
    let mut segment = WorkflowMock::with_replay::<RebuildSegmentAnalytics>(
        RebuildInput::new(segment_change(), DebouncePolicy::default()),
        true,
    );
    drain(&mut segment);
    segment.result::<()>();
    let mut heatmap = WorkflowMock::with_replay::<PrepareHeatmap>(
        RebuildInput::new(heatmap_change(), DebouncePolicy::default()),
        true,
    );
    drain(&mut heatmap);
    heatmap.result::<()>();
}
