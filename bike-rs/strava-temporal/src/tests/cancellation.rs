use super::{collection, fixture::Fixture, runtime::WorkflowMock};
use crate::{activities::StravaActivities, workflows::*};
use integrations::strava::{events::StravaOperation, native::*};
use temporalio_common::protos::coresdk::workflow_commands::workflow_command;
use temporalio_workflow::common::WorkflowDefinition;

fn scheduled(workflow: &mut WorkflowMock, name: &str) -> u32 {
    let workflow_command::Variant::ScheduleActivity(activity) = workflow.next_command() else {
        panic!("Expected Activity {name}")
    };
    assert_eq!(activity.activity_type, name);
    activity.seq
}

fn prepare(workflow: &mut WorkflowMock, fixture: &Fixture) {
    workflow.activity(
        StravaActivities::fetch_activity.name(),
        &fixture.event,
        &fixture.activity(),
    );
    workflow.activity(
        StravaActivities::fetch_streams.name(),
        &fixture.activity(),
        &fixture.sources(),
    );
    workflow.activity(
        StravaActivities::normalize_activity.name(),
        &fixture.sources(),
        &fixture.normalized(),
    );
}

#[test]
fn cancellation_before_apply_prevents_mutation_even_after_preparation_succeeds() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    workflow.activity(
        StravaActivities::fetch_activity.name(),
        &fixture.event,
        &fixture.activity(),
    );
    workflow.activity(
        StravaActivities::fetch_streams.name(),
        &fixture.activity(),
        &fixture.sources(),
    );
    let seq = scheduled(&mut workflow, StravaActivities::normalize_activity.name());
    workflow.cancel();
    assert!(matches!(
        workflow.next_command(),
        workflow_command::Variant::RequestCancelActivity(_)
    ));
    workflow.complete_activity(seq, &fixture.normalized());
    workflow.assert_cancelled();
    workflow.assert_no_commands();
}

#[test]
fn late_upsert_cancellation_settles_the_source_and_every_remaining_publication() {
    for cancel_after_commit in [false, true] {
        let fixture = Fixture::reconciling(StravaOperation::Upsert);
        let change = fixture.change(ActivityStatus::Applied);
        let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
        prepare(&mut workflow, &fixture);
        let seq = scheduled(&mut workflow, StravaActivities::persist_activity.name());
        if !cancel_after_commit {
            workflow.cancel();
            workflow.assert_no_commands();
        }
        workflow.complete_activity(seq, &change);
        workflow.activity(StravaActivities::rebuild_activity.name(), &change, &change);
        let seq = scheduled(&mut workflow, StravaActivities::rebuild_segments.name());
        if cancel_after_commit {
            workflow.cancel();
            workflow.assert_no_commands();
        }
        workflow.complete_activity(seq, &change);
        let mut workflow = workflow.restart::<ReconcileActivity>(fixture.event.clone());
        for activity in [
            StravaActivities::rebuild_fitness.name(),
            StravaActivities::publish_heatmap.name(),
            StravaActivities::complete_activity.name(),
        ] {
            workflow.activity(activity, &change, &change);
        }
        assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Applied);
    }
}

#[test]
fn late_delete_cancellation_settles_the_mutation_and_cleanup() {
    let fixture = Fixture::reconciling(StravaOperation::Delete);
    let change = fixture.change(ActivityStatus::Deleted);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    workflow.activity(StravaActivities::capture_deletion.name(), &fixture.event, &change);
    let seq = scheduled(&mut workflow, StravaActivities::delete_activity.name());
    workflow.cancel();
    workflow.assert_no_commands();
    workflow.complete_activity(seq, &change);
    for activity in [
        StravaActivities::delete_source.name(),
        StravaActivities::rebuild_segments.name(),
        StravaActivities::rebuild_fitness.name(),
        StravaActivities::complete_activity.name(),
    ] {
        workflow.activity(activity, &change, &change);
    }
    assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Deleted);
}

#[test]
fn single_event_parents_wait_for_the_childs_cancellation_decision() {
    for operation in [
        StravaOperation::Create,
        StravaOperation::Update,
        StravaOperation::Upsert,
        StravaOperation::Delete,
    ] {
        for child_committed in [false, true] {
            let fixture = Fixture::new(operation);
            let (mut workflow, child_type) = match operation {
                StravaOperation::Create => (
                    WorkflowMock::new::<CreateActivity>(fixture.event.clone()),
                    UpsertActivity::run.name(),
                ),
                StravaOperation::Update => (
                    WorkflowMock::new::<UpdateActivity>(fixture.event.clone()),
                    UpsertActivity::run.name(),
                ),
                StravaOperation::Delete => (
                    WorkflowMock::new::<DeleteActivity>(fixture.event.clone()),
                    ReconcileActivity::run.name(),
                ),
                _ => (
                    WorkflowMock::new::<UpsertActivity>(fixture.event.clone()),
                    ReconcileActivity::run.name(),
                ),
            };
            let seq = workflow.start_child(child_type, &fixture.event);
            if child_committed {
                workflow.cancel();
                assert!(matches!(
                    workflow.next_command(),
                    workflow_command::Variant::CancelChildWorkflowExecution(_)
                ));
                workflow.complete_child(seq, &fixture.change(ActivityStatus::Applied));
                assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Applied);
            } else {
                workflow.cancel_child(seq);
                workflow.assert_cancelled();
            }
            workflow.assert_no_commands();
        }
    }
}

#[test]
fn cancelled_collection_settles_current_child_and_starts_no_later_items() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = WorkflowMock::new::<SyncActivities>(fixture.event.clone());
    let input = collection::open_collection(&mut workflow, &fixture);
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input.clone(),
            page: 1,
        },
        &ActivityPage {
            activity_ids: vec![fixture.activity_id, fixture.activity_id + 1],
            next_page: None,
            latest_started_at: None,
        },
    );
    let first = collection::accept_child(&mut workflow, &input.event, fixture.activity_id);
    let seq = workflow.start_child(UpsertActivity::run.name(), &first);
    workflow.cancel();
    assert!(matches!(
        workflow.next_command(),
        workflow_command::Variant::CancelChildWorkflowExecution(_)
    ));
    workflow.complete_child(seq, &fixture.change(ActivityStatus::Applied));
    workflow.assert_cancelled();
    workflow.assert_no_commands();
}

#[test]
fn retained_inline_history_preserves_its_cancellable_mutation() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::with_replay::<UpsertActivity>(fixture.event.clone(), true);
    prepare(&mut workflow, &fixture);
    workflow.cancel_activity(StravaActivities::persist_activity.name());
    workflow.assert_cancelled();
}

#[test]
fn late_cancellation_preserves_a_single_events_child_failure() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event.clone());
    let seq = workflow.start_child(ReconcileActivity::run.name(), &fixture.event);
    workflow.cancel();
    assert!(matches!(
        workflow.next_command(),
        workflow_command::Variant::CancelChildWorkflowExecution(_)
    ));
    workflow.fail_child(seq);
    workflow.assert_failed();
}
