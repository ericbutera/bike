use super::{fixture::Fixture, runtime::WorkflowMock};
use crate::activities::StravaActivities;
use crate::workflows::{DeleteActivity, DisconnectStrava};
use integrations::strava::native::ActivityStatus;
use integrations::strava::{
    events::{StravaEvent, StravaOperation},
    native::ChildEvent,
};
use temporalio_workflow::common::WorkflowDefinition;

#[test]
fn disconnect_waits_for_owned_activity_deletion_before_removing_the_connection() {
    let fixture = Fixture::new(StravaOperation::Disconnect);
    let mut workflow = WorkflowMock::new::<DisconnectStrava>(fixture.event.clone());
    workflow.activity(
        StravaActivities::begin_disconnect.name(),
        &fixture.event,
        &vec![fixture.activity_id],
    );
    let child = fixture.event.for_activity(fixture.activity_id, StravaOperation::Delete);
    workflow.activity(
        StravaActivities::accept_child.name(),
        &ChildEvent {
            parent: fixture.event.clone(),
            child: child.clone(),
        },
        &child,
    );
    let seq = workflow.start_child(DeleteActivity::run.name(), &child);
    workflow.complete_child(seq, &fixture.change(ActivityStatus::Deleted));
    workflow.activity(
        StravaActivities::revoke_connection.name(),
        &fixture.event,
        &fixture.event,
    );
    workflow.activity(
        StravaActivities::remove_connection.name(),
        &fixture.event,
        &fixture.event,
    );
    assert!(!workflow.result::<StravaEvent>().revoke_remote);
}

#[test]
fn deletion_failure_keeps_the_connection_for_retry() {
    let fixture = Fixture::new(StravaOperation::Disconnect);
    let mut workflow = WorkflowMock::new::<DisconnectStrava>(fixture.event.clone());
    workflow.activity(
        StravaActivities::begin_disconnect.name(),
        &fixture.event,
        &vec![fixture.activity_id],
    );
    let child = fixture.event.for_activity(fixture.activity_id, StravaOperation::Delete);
    workflow.activity(
        StravaActivities::accept_child.name(),
        &ChildEvent {
            parent: fixture.event.clone(),
            child: child.clone(),
        },
        &child,
    );
    let seq = workflow.start_child(DeleteActivity::run.name(), &child);
    workflow.fail_child(seq);
    workflow.assert_failed();
}
