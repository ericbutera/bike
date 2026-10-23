use super::{fixture::Fixture, runtime::WorkflowMock};
use crate::activities::StravaActivities;
use crate::workflows::*;
use integrations::strava::native::ActivityStatus;
use integrations::strava::{events::StravaOperation, native::ActivityChange};
use temporalio_workflow::common::WorkflowDefinition;

#[test]
fn restarted_upsert_replays_persisted_steps_and_resumes_publication() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let change = fixture.change(ActivityStatus::Applied);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    persist(&mut workflow, &fixture, &change);
    let mut workflow = workflow.restart::<ReconcileActivity>(fixture.event.clone());
    for activity in [
        StravaActivities::rebuild_activity.name(),
        StravaActivities::rebuild_segments.name(),
        StravaActivities::rebuild_fitness.name(),
        StravaActivities::publish_heatmap.name(),
        StravaActivities::complete_activity.name(),
    ] {
        workflow.activity(activity, &change, &change);
    }
    assert_eq!(workflow.result::<ActivityChange>().bike_activity_id, Some(42));
}

#[test]
fn create_uses_an_upsert_child_and_returns_its_bike_activity_id() {
    let fixture = Fixture::new(StravaOperation::Create);
    let mut workflow = WorkflowMock::new::<CreateActivity>(fixture.event.clone());
    let child = workflow.start_child(UpsertActivity::run.name(), &fixture.event);
    workflow.complete_child(child, &fixture.change(ActivityStatus::Applied));
    assert_eq!(workflow.result::<ActivityChange>().bike_activity_id, Some(42));
}

#[test]
fn update_uses_an_upsert_child_and_preserves_customer_and_activity_context() {
    let fixture = Fixture::new(StravaOperation::Update);
    let mut workflow = WorkflowMock::new::<UpdateActivity>(fixture.event.clone());
    let child = workflow.start_child(UpsertActivity::run.name(), &fixture.event);
    workflow.complete_child(child, &fixture.change(ActivityStatus::Applied));
    let change = workflow.result::<ActivityChange>();
    assert_eq!(change.event.correlation_id, fixture.event.correlation_id);
    assert_eq!(change.event.strava_activity_id, Some(fixture.activity_id));
}

#[test]
fn upsert_completes_only_after_activity_and_customer_results_are_published() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    persist(&mut workflow, &fixture, &fixture.change(ActivityStatus::Applied));
    let change = fixture.change(ActivityStatus::Applied);
    for activity in [
        StravaActivities::rebuild_activity.name(),
        StravaActivities::rebuild_segments.name(),
        StravaActivities::rebuild_fitness.name(),
        StravaActivities::publish_heatmap.name(),
        StravaActivities::complete_activity.name(),
    ] {
        workflow.activity(activity, &change, &change);
    }
    assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Applied);
}

#[test]
fn superseded_upsert_does_not_publish_or_complete_an_import() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    persist(&mut workflow, &fixture, &fixture.change(ActivityStatus::Superseded));
    assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Superseded);
}

#[test]
fn source_snapshot_change_refetches_instead_of_publishing_stale_data() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    persist(&mut workflow, &fixture, &fixture.change(ActivityStatus::SourceChanged));
    persist(&mut workflow, &fixture, &fixture.change(ActivityStatus::Superseded));
    assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Superseded);
}

#[test]
fn upsert_and_delete_delegate_to_the_same_owned_source_key() {
    for operation in [StravaOperation::Upsert, StravaOperation::Delete] {
        let fixture = Fixture::new(operation);
        let mut workflow = if operation == StravaOperation::Delete {
            WorkflowMock::new::<DeleteActivity>(fixture.event.clone())
        } else {
            WorkflowMock::new::<UpsertActivity>(fixture.event.clone())
        };
        let temporalio_common::protos::coresdk::workflow_commands::workflow_command::Variant::StartChildWorkflowExecution(command)
            = workflow.next_command() else { panic!("Expected source child") };
        assert_eq!(command.workflow_id, fixture.event.source_workflow_id().unwrap());
        assert_eq!(command.workflow_type, ReconcileActivity::run.name());
        assert_eq!(
            command.workflow_id_reuse_policy,
            temporalio_sdk::WorkflowIdReusePolicy::AllowDuplicate as i32
        );
        assert_eq!(
            command.parent_close_policy,
            temporalio_sdk::ParentClosePolicy::RequestCancel as i32
        );
    }
}

#[test]
fn distinct_event_waits_for_a_busy_source_then_starts_its_own_reconciliation() {
    use temporalio_common::protos::coresdk::{
        workflow_activation::{workflow_activation_job::Variant, FireTimer},
        workflow_commands::workflow_command,
    };
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event.clone());
    workflow.fail_child_start(ReconcileActivity::run.name(), &fixture.event);
    let workflow_command::Variant::StartTimer(timer) = workflow.next_command() else {
        panic!("Expected durable retry timer")
    };
    assert_eq!(timer.start_to_fire_timeout.unwrap().seconds, 1);
    workflow.activate(Variant::FireTimer(FireTimer { seq: timer.seq }));
    let child = workflow.start_child(ReconcileActivity::run.name(), &fixture.event);
    workflow.complete_child(child, &fixture.change(ActivityStatus::Applied));
    assert_eq!(
        workflow.result::<ActivityChange>().event.strava_event_id,
        fixture.event.strava_event_id
    );
}

#[test]
fn busy_source_continues_with_original_input_before_history_grows_unbounded() {
    use temporalio_common::protos::coresdk::{
        workflow_activation::{workflow_activation_job::Variant, FireTimer},
        workflow_commands::workflow_command,
    };
    for operation in [StravaOperation::Upsert, StravaOperation::Delete] {
        for suggested in [false, true] {
            let fixture = Fixture::new(operation);
            let mut workflow = if operation == StravaOperation::Delete {
                WorkflowMock::new::<DeleteActivity>(fixture.event.clone())
            } else {
                WorkflowMock::new::<UpsertActivity>(fixture.event.clone())
            };
            let attempts = if suggested { 1 } else { 128 };
            for attempt in 0..attempts {
                workflow.fail_child_start(ReconcileActivity::run.name(), &fixture.event);
                let workflow_command::Variant::StartTimer(timer) = workflow.next_command() else {
                    panic!("Expected durable retry timer")
                };
                assert_eq!(
                    timer.start_to_fire_timeout.unwrap().seconds,
                    (1_i64 << attempt.min(5)).min(30)
                );
                if suggested {
                    workflow.suggest_continue_as_new();
                }
                workflow.activate(Variant::FireTimer(FireTimer { seq: timer.seq }));
            }
            let continued = workflow.continued_input::<integrations::strava::events::StravaEvent>();
            assert_eq!(
                serde_json::to_value(&continued).unwrap(),
                serde_json::to_value(&fixture.event).unwrap()
            );
            let mut resumed = if operation == StravaOperation::Delete {
                WorkflowMock::new::<DeleteActivity>(continued.clone())
            } else {
                WorkflowMock::new::<UpsertActivity>(continued.clone())
            };
            let child = resumed.start_child(ReconcileActivity::run.name(), &continued);
            resumed.complete_child(child, &fixture.change(ActivityStatus::Applied));
            assert_eq!(
                resumed.result::<ActivityChange>().event.strava_event_id,
                fixture.event.strava_event_id
            );
        }
    }
}

#[test]
fn cancellation_while_waiting_for_a_source_does_not_launch_another_child() {
    use temporalio_common::protos::coresdk::workflow_commands::workflow_command;
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event.clone());
    workflow.fail_child_start(ReconcileActivity::run.name(), &fixture.event);
    assert!(matches!(
        workflow.next_command(),
        workflow_command::Variant::StartTimer(_)
    ));
    workflow.cancel();
    assert!(matches!(
        workflow.next_command(),
        workflow_command::Variant::CancelTimer(_)
    ));
    workflow.assert_cancelled();
    workflow.assert_no_commands();
}

#[test]
fn cancelled_source_request_never_starts_provider_preparation() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let workflow = WorkflowMock::cancelled_at_start::<UpsertActivity>(fixture.event);
    workflow.assert_cancelled();
    workflow.assert_no_commands();
}

#[test]
fn excluded_activity_cleans_up_and_rebuilds_without_publishing_a_heatmap() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut change = fixture.change(ActivityStatus::Deleted);
    change.removed_source = Some("fixture:retired-source".into());
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    persist(&mut workflow, &fixture, &change);
    for activity in [
        StravaActivities::delete_source.name(),
        StravaActivities::rebuild_segments.name(),
        StravaActivities::rebuild_fitness.name(),
    ] {
        workflow.activity(activity, &change, &change);
    }
    assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Deleted);
}

#[test]
fn provider_failure_stops_before_persistence_and_publication() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event);
    workflow.fail_activity(StravaActivities::fetch_activity.name(), "Fixture Strava failure");
    workflow.assert_failed();
}

#[test]
fn cancellation_cannot_be_reported_as_success() {
    let fixture = Fixture::reconciling(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event);
    workflow.cancel_activity(StravaActivities::fetch_activity.name());
    workflow.assert_cancelled();
}

#[test]
fn delete_captures_dependencies_and_rebuilds_before_completion() {
    let fixture = Fixture::reconciling(StravaOperation::Delete);
    let change = fixture.change(ActivityStatus::Deleted);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    workflow.activity(StravaActivities::capture_deletion.name(), &fixture.event, &change);
    for activity in [
        StravaActivities::delete_activity.name(),
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
fn superseded_delete_does_not_remove_sources_or_rebuild_results() {
    let fixture = Fixture::reconciling(StravaOperation::Delete);
    let mut workflow = WorkflowMock::new::<ReconcileActivity>(fixture.event.clone());
    workflow.activity(
        StravaActivities::capture_deletion.name(),
        &fixture.event,
        &fixture.change(ActivityStatus::Deleted),
    );
    workflow.activity(
        StravaActivities::delete_activity.name(),
        &fixture.change(ActivityStatus::Deleted),
        &fixture.change(ActivityStatus::Superseded),
    );
    assert_eq!(workflow.result::<ActivityChange>().status, ActivityStatus::Superseded);
}

fn persist(workflow: &mut WorkflowMock, fixture: &Fixture, change: &ActivityChange) {
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
    workflow.activity(StravaActivities::persist_activity.name(), &fixture.normalized(), change);
}
