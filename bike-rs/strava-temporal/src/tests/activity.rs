use super::{fixture::Fixture, logging::Logs, runtime::WorkflowMock};
use crate::activities::StravaActivities;
use crate::workflows::*;
use bike_core::strava::native::ActivityStatus;
use bike_core::strava::{events::StravaOperation, native::ActivityChange};
use temporalio_workflow::common::WorkflowDefinition;

#[test]
fn create_uses_an_upsert_child_and_returns_its_bike_activity_id() {
    let fixture = Fixture::new(StravaOperation::Create);
    let mut workflow = WorkflowMock::new::<CreateActivity>(fixture.event.clone());
    let child = workflow.start_child(UpsertActivity::run.name(), &fixture.event);
    workflow.complete_child(child, &fixture.change(ActivityStatus::Applied));
    assert_eq!(
        workflow.result::<ActivityChange>().bike_activity_id,
        Some(42)
    );
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
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event.clone());
    persist(
        &mut workflow,
        &fixture,
        &fixture.change(ActivityStatus::Applied),
    );
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
    assert_eq!(
        workflow.result::<ActivityChange>().status,
        ActivityStatus::Applied
    );
}

#[test]
fn superseded_upsert_does_not_publish_or_complete_an_import() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event.clone());
    persist(
        &mut workflow,
        &fixture,
        &fixture.change(ActivityStatus::Superseded),
    );
    assert_eq!(
        workflow.result::<ActivityChange>().status,
        ActivityStatus::Superseded
    );
}

#[test]
fn excluded_activity_cleans_up_and_rebuilds_without_publishing_a_heatmap() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut change = fixture.change(ActivityStatus::Deleted);
    change.removed_source = Some("fixture:retired-source".into());
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event.clone());
    persist(&mut workflow, &fixture, &change);
    for activity in [
        StravaActivities::delete_source.name(),
        StravaActivities::rebuild_segments.name(),
        StravaActivities::rebuild_fitness.name(),
    ] {
        workflow.activity(activity, &change, &change);
    }
    assert_eq!(
        workflow.result::<ActivityChange>().status,
        ActivityStatus::Deleted
    );
}

#[test]
fn provider_failure_stops_before_persistence_and_publication() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event);
    workflow.fail_activity(
        StravaActivities::fetch_activity.name(),
        "Fixture Strava failure",
    );
    workflow.assert_failed();
}

#[test]
fn cancellation_cannot_be_reported_as_success() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let logs = Logs::default();
    logs.capture(|| {
        let mut workflow = WorkflowMock::new::<UpsertActivity>(fixture.event);
        workflow.cancel_activity(StravaActivities::fetch_activity.name());
        workflow.assert_cancelled();
    });
    assert_eq!(logs.workflow("unit-workflow", "cancelled")["level"], "INFO");
    assert!(logs
        .rows()
        .iter()
        .all(|row| row["fields"]["status"] != "completed"));
}

#[test]
fn delete_captures_dependencies_and_rebuilds_before_completion() {
    let fixture = Fixture::new(StravaOperation::Delete);
    let change = fixture.change(ActivityStatus::Deleted);
    let mut workflow = WorkflowMock::new::<DeleteActivity>(fixture.event.clone());
    workflow.activity(
        StravaActivities::capture_deletion.name(),
        &fixture.event,
        &change,
    );
    for activity in [
        StravaActivities::delete_activity.name(),
        StravaActivities::delete_source.name(),
        StravaActivities::rebuild_segments.name(),
        StravaActivities::rebuild_fitness.name(),
        StravaActivities::complete_activity.name(),
    ] {
        workflow.activity(activity, &change, &change);
    }
    assert_eq!(
        workflow.result::<ActivityChange>().status,
        ActivityStatus::Deleted
    );
}

#[test]
fn superseded_delete_does_not_remove_sources_or_rebuild_results() {
    let fixture = Fixture::new(StravaOperation::Delete);
    let mut workflow = WorkflowMock::new::<DeleteActivity>(fixture.event.clone());
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
    assert_eq!(
        workflow.result::<ActivityChange>().status,
        ActivityStatus::Superseded
    );
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
    workflow.activity(
        StravaActivities::persist_activity.name(),
        &fixture.normalized(),
        change,
    );
}
