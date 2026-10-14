use super::{
    collection::{accept_child, open_collection},
    fixture::Fixture,
    runtime::WorkflowMock,
};
use crate::{activities::StravaActivities, workflows::*};
use bike_core::strava::{events::StravaOperation, native::*};
use temporalio_workflow::common::WorkflowDefinition;

#[test]
fn server_history_suggestion_preserves_unprocessed_ids_inside_a_populated_page() {
    let mut fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = collection_workflow(&fixture);
    let input = open_collection(&mut workflow, &fixture);
    let ids = vec![fixture.activity_id, fixture.activity_id + 1];
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input.clone(),
            page: 1,
        },
        &ActivityPage {
            activity_ids: ids.clone(),
            next_page: None,
            latest_started_at: None,
        },
    );
    let first = accept_child(&mut workflow, &input.event, ids[0]);
    let seq = workflow.start_child(UpsertActivity::run.name(), &first);
    let mut change = fixture.change(ActivityStatus::Applied);
    change.event = first;
    workflow.suggest_continue_as_new();
    let mut workflow = workflow.restart::<SyncActivities>(fixture.event.clone());
    workflow.complete_child(seq, &change);
    fixture.event = workflow.continued_input::<bike_core::strava::events::StravaEvent>();
    let progress = fixture.event.progress.as_ref().unwrap();
    assert_eq!(progress.imported, 1);
    assert_eq!(
        progress.pending_page.as_ref().unwrap().activity_ids,
        vec![ids[1]]
    );
    let mut workflow = collection_workflow(&fixture);
    let input = open_collection(&mut workflow, &fixture);
    // The remaining ID comes from history; do not re-list a potentially changed page.
    let second = accept_child(&mut workflow, &input.event, ids[1]);
    let seq = workflow.start_child(UpsertActivity::run.name(), &second);
    let mut change = fixture.change(ActivityStatus::Unchanged);
    change.event = second;
    workflow.complete_child(seq, &change);
    let expected = CollectionResult {
        event: input.event,
        imported: 1,
        unchanged: 1,
        last_started_at: None,
    };
    workflow.activity(StravaActivities::complete_sync.name(), &expected, &expected);
    let result = workflow.result::<CollectionResult>();
    assert_eq!(result.imported + result.unchanged, 2);
}

#[test]
fn pre_patch_history_finishes_the_populated_page_before_continuing() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = WorkflowMock::with_replay::<SyncActivities>(fixture.event.clone(), true);
    let input = open_collection(&mut workflow, &fixture);
    let ids = vec![fixture.activity_id, fixture.activity_id + 1];
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input.clone(),
            page: 1,
        },
        &ActivityPage {
            activity_ids: ids.clone(),
            next_page: Some(2),
            latest_started_at: None,
        },
    );
    workflow.suggest_continue_as_new();
    for id in ids {
        let event = accept_child(&mut workflow, &input.event, id);
        let seq = workflow.start_child(UpsertActivity::run.name(), &event);
        let mut change = fixture.change(ActivityStatus::Applied);
        change.event = event;
        workflow.complete_child(seq, &change);
    }
    let continued = workflow.continued_input::<bike_core::strava::events::StravaEvent>();
    let progress = continued.progress.unwrap();
    assert_eq!(progress.imported, 2);
    assert_eq!(progress.next_page, 2);
    assert!(progress.pending_page.is_none());
}

#[test]
fn large_sync_and_backfill_preserve_counts_and_windows_across_three_runs() {
    for operation in [StravaOperation::Sync, StravaOperation::Backfill] {
        large_collection(operation);
    }
}

fn large_collection(operation: StravaOperation) {
    let mut fixture = Fixture::new(operation);
    let original = fixture.event.clone();
    let latest = chrono::DateTime::from_timestamp(1790856000, 0).unwrap();
    for first_page in [1, 26, 51] {
        let mut workflow = collection_workflow(&fixture);
        let input = open_collection(&mut workflow, &fixture);
        for page in first_page..=51.min(first_page + 24) {
            collection_page(&mut workflow, &fixture, &input, page, latest);
        }
        if first_page < 51 {
            let continued = workflow.continued_input::<bike_core::strava::events::StravaEvent>();
            let progress = continued.progress.as_ref().unwrap();
            assert_eq!(progress.next_page, first_page + 25);
            assert_eq!(progress.imported, (first_page + 24) * 50);
            assert_eq!(progress.unchanged, progress.imported);
            assert_eq!(progress.last_started_at, Some(latest));
            assert_eq!(continued.window.as_ref().unwrap().after, input.window.after);
            assert_eq!(
                continued.window.as_ref().unwrap().before,
                input.window.before
            );
            assert_eq!(continued.strava_event_id, original.strava_event_id);
            assert_eq!(continued.correlation_id, original.correlation_id);
            assert_eq!(continued.request_id, original.request_id);
            assert_eq!(continued.trace_context, original.trace_context);
            fixture.event = continued;
        } else {
            let expected = CollectionResult {
                event: input.event,
                imported: 2550,
                unchanged: 2550,
                last_started_at: Some(latest),
            };
            let completion = match operation {
                StravaOperation::Sync => StravaActivities::complete_sync.name(),
                _ => StravaActivities::complete_backfill.name(),
            };
            workflow.activity(completion, &expected, &expected);
            let result = workflow.result::<CollectionResult>();
            assert_eq!(result.imported + result.unchanged, 5100);
            assert_eq!(result.last_started_at, Some(latest));
        }
    }
}

fn collection_workflow(fixture: &Fixture) -> WorkflowMock {
    match fixture.event.operation {
        StravaOperation::Sync => WorkflowMock::new::<SyncActivities>(fixture.event.clone()),
        _ => WorkflowMock::new::<BackfillActivities>(fixture.event.clone()),
    }
}

fn collection_page(
    workflow: &mut WorkflowMock,
    fixture: &Fixture,
    input: &CollectionInput,
    page: i32,
    latest: chrono::DateTime<chrono::Utc>,
) {
    let ids: Vec<_> = (0..100).map(|i| i64::from(page) * 100 + i).collect();
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input.clone(),
            page,
        },
        &ActivityPage {
            activity_ids: ids.clone(),
            next_page: (page < 51).then_some(page + 1),
            latest_started_at: Some(latest),
        },
    );
    for id in ids {
        let child = accept_child(workflow, &input.event, id);
        let seq = workflow.start_child(UpsertActivity::run.name(), &child);
        let status = if id % 2 == 0 {
            ActivityStatus::Applied
        } else {
            ActivityStatus::Unchanged
        };
        let mut change = fixture.change(status);
        change.event = child;
        workflow.complete_child(seq, &change);
    }
}

#[test]
fn server_history_suggestion_continues_before_the_page_cap() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = collection_workflow(&fixture);
    let input = open_collection(&mut workflow, &fixture);
    workflow.suggest_continue_as_new();
    workflow.activity(
        StravaActivities::list_activity_page.name(),
        &PageInput {
            collection: input,
            page: 1,
        },
        &ActivityPage {
            activity_ids: vec![],
            next_page: Some(2),
            latest_started_at: None,
        },
    );
    let continued = workflow.continued_input::<bike_core::strava::events::StravaEvent>();
    assert_eq!(continued.progress.unwrap().next_page, 2);
}

#[test]
fn restart_while_a_customer_child_is_running_preserves_its_identity_and_offset() {
    let fixture = Fixture::new(StravaOperation::Sync);
    let mut workflow = collection_workflow(&fixture);
    let input = open_collection(&mut workflow, &fixture);
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
    let mut workflow = workflow.restart::<SyncActivities>(fixture.event.clone());
    let mut change = fixture.change(ActivityStatus::Applied);
    change.event = child;
    workflow.complete_child(seq, &change);
    let expected = CollectionResult {
        event: input.event,
        imported: 1,
        unchanged: 0,
        last_started_at: None,
    };
    workflow.activity(StravaActivities::complete_sync.name(), &expected, &expected);
    assert_eq!(workflow.result::<CollectionResult>().imported, 1);
}
