use super::{fixture::Fixture, runtime::WorkflowMock};
use crate::activities::StravaActivities;
use crate::workflows::*;
use integrations::strava::{
    events::{StravaEvent, StravaOperation},
    native::*,
};
use temporalio_workflow::common::WorkflowDefinition;

#[test]
fn sync_counts_children_and_commits_the_latest_offset_after_publication() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = WorkflowMock::new::<SyncActivities>(fixture.event.clone());
    verify_collection(&mut workflow, &fixture, StravaActivities::complete_sync.name());
}

#[test]
fn backfill_finishes_its_date_range_without_calling_the_sync_commit() {
    let fixture = Fixture::new(StravaOperation::Backfill);
    let mut workflow = WorkflowMock::new::<BackfillActivities>(fixture.event.clone());
    verify_collection(&mut workflow, &fixture, StravaActivities::complete_backfill.name());
}

#[test]
fn child_failure_does_not_commit_a_sync_offset() {
    let fixture = Fixture::new(StravaOperation::Sync);
    fail_collection(&fixture);
}

fn fail_collection(fixture: &Fixture) {
    let mut workflow = WorkflowMock::new::<SyncActivities>(fixture.event.clone());
    let input = open_collection(&mut workflow, fixture);
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input.clone(),
            page: 1,
        },
        &ActivityPage {
            activity_ids: vec![fixture.activity_id],
            next_page: None,
            latest_started_at: None,
        },
    );
    let child = accept_child(&mut workflow, &input.event, fixture.activity_id);
    let seq = workflow.start_child(UpsertActivity::run.name(), &child);
    workflow.fail_child(seq);
    workflow.assert_failed();
}

#[test]
fn pagination_continues_as_new_with_the_same_window_and_next_page() {
    let fixture = Fixture::new(StravaOperation::Sync);
    continue_collection(&fixture);
}

fn continue_collection(fixture: &Fixture) {
    let mut workflow = WorkflowMock::new::<SyncActivities>(fixture.event.clone());
    let input = open_collection(&mut workflow, fixture);
    for page in 1..=25 {
        workflow.activity(
            StravaActivities::list_activity_page.name(),
            &PageInput {
                collection: input.clone(),
                page,
            },
            &ActivityPage {
                activity_ids: vec![],
                next_page: Some(page + 1),
                latest_started_at: None,
            },
        );
    }
    let continued = workflow.continued_input::<StravaEvent>();
    assert_eq!(continued.progress.unwrap().next_page, 26);
    assert_eq!(continued.window.unwrap().after, input.window.after);
    assert_eq!(continued.correlation_id, fixture.event.correlation_id);
    assert_eq!(continued.request_id, fixture.event.request_id);
    assert_eq!(continued.trace_context, fixture.event.trace_context);
    assert_eq!(continued.strava_event_id, fixture.event.strava_event_id);
}

#[test]
fn replay_preserves_commands_and_child_inputs() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = WorkflowMock::with_replay::<SyncActivities>(fixture.event.clone(), true);
    verify_collection(&mut workflow, &fixture, StravaActivities::complete_sync.name());
}

pub(super) fn open_collection(workflow: &mut WorkflowMock, fixture: &Fixture) -> CollectionInput {
    let mut input = fixture.collection();
    workflow.activity(StravaActivities::collection_window.name(), &fixture.event, &input);
    input.event.window = Some(input.window.clone());
    input
}

pub(super) fn accept_child(workflow: &mut WorkflowMock, parent: &StravaEvent, id: i64) -> StravaEvent {
    let child = parent.for_activity(id, StravaOperation::Upsert);
    workflow.activity(
        StravaActivities::accept_child.name(),
        &ChildEvent {
            parent: parent.clone(),
            child: child.clone(),
        },
        &child,
    );
    child
}

fn verify_collection(workflow: &mut WorkflowMock, fixture: &Fixture, completion: &str) {
    let input = open_collection(workflow, fixture);
    let latest = chrono::DateTime::parse_from_rfc3339("2026-10-01T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input.clone(),
            page: 1,
        },
        &ActivityPage {
            activity_ids: vec![fixture.activity_id, fixture.activity_id + 1],
            next_page: None,
            latest_started_at: Some(latest),
        },
    );
    for (id, status) in [
        (fixture.activity_id, ActivityStatus::Applied),
        (fixture.activity_id + 1, ActivityStatus::Unchanged),
    ] {
        let child = accept_child(workflow, &input.event, id);
        let seq = workflow.start_child(UpsertActivity::run.name(), &child);
        let mut change = fixture.change(status);
        change.event = child;
        workflow.complete_child(seq, &change);
    }
    let expected = CollectionResult {
        event: input.event,
        imported: 1,
        unchanged: 1,
        last_started_at: Some(latest),
    };
    workflow.activity(completion, &expected, &expected);
    let result = workflow.result::<CollectionResult>();
    assert_eq!(result.imported, 1);
    assert_eq!(result.unchanged, 1);
    assert_eq!(result.last_started_at, Some(latest));
}
