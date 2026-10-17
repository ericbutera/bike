use super::batches::{children, page};
use super::*;
use crate::workflows::{RegenerateActivitySegments, RegenerateSegmentEfforts, RegenerateUserSegments};
use bike_core::{bike_tasks::*, test_support as fixtures};

#[test]
fn activity_matching_hands_off_to_the_existing_segment_analytics_workflow() {
    let request = fixtures::page_request(ManifestScope::UserSegments { user_id: 7 }, 1, 0);
    let command = fixtures::segment_entry(&request, 0);
    let result = fixtures::segment_result();
    let mut mock = WorkflowMock::new::<RegenerateActivitySegments>(command.clone());
    mock.activity("match_activity_segments", &command, &result);
    mock.activity("schedule_segment_analytics", &result.segments, &());
    assert_eq!(mock.result::<ActivitySegmentsResult>(), result);
}

#[test]
fn matching_failure_never_requests_segment_analytics() {
    let request = fixtures::page_request(ManifestScope::UserSegments { user_id: 7 }, 1, 0);
    let mut mock = WorkflowMock::new::<RegenerateActivitySegments>(fixtures::segment_entry(&request, 0));
    mock.fail_activity("match_activity_segments", "matching failed");
    mock.assert_failure_message("matching failed");
}

#[test]
fn user_regeneration_reuses_per_activity_segment_children() {
    let input = fixtures::batch_input();
    let request = fixtures::page_request(ManifestScope::UserSegments { user_id: 7 }, 2, 0);
    let mut mock = WorkflowMock::new::<RegenerateUserSegments>(input.clone());
    mock.activity("prepare_user_segments", &input.command, &request.manifest);
    let entries = page(&request, fixtures::segment_entry);
    mock.activity("segment_activity_page", &request, &entries);
    children(
        &mut mock,
        "regenerate_activity_segments",
        &entries,
        &fixtures::segment_result(),
    );
    assert_eq!(mock.result::<BatchResult>().completed, 2);
}

#[test]
fn segment_regeneration_reuses_the_same_children_with_one_segment_selection() {
    let input = fixtures::segment_efforts_input();
    let request = fixtures::page_request(
        ManifestScope::Segment {
            segment_id: 18,
            expected_revision: 1,
        },
        2,
        0,
    );
    let mut mock = WorkflowMock::new::<RegenerateSegmentEfforts>(input.clone());
    mock.activity(
        "prepare_segment_efforts",
        &SegmentEffortsSelection {
            command: input.batch.command,
            segment_id: 18,
            expected_revision: 1,
        },
        &request.manifest,
    );
    let mut entries = page(&request, fixtures::segment_entry);
    // A global segment may include activities belonging to several users.
    entries[1].command.user_id = 8;
    mock.activity("segment_activity_page", &request, &entries);
    children(
        &mut mock,
        "regenerate_activity_segments",
        &entries,
        &fixtures::segment_result(),
    );
    assert_eq!(mock.result::<BatchResult>().completed, 2);
}

#[test]
fn segment_manifest_cannot_request_all_segments_or_a_different_generation() {
    let request = fixtures::page_request(
        ManifestScope::Segment {
            segment_id: 18,
            expected_revision: 1,
        },
        1,
        0,
    );
    for selection in [
        SegmentSelection::All,
        SegmentSelection::One {
            segment_id: 18,
            expected_revision: 2,
        },
    ] {
        let mut input = fixtures::segment_efforts_input();
        input.batch = fixtures::prepared_batch(&request);
        let mut entry = fixtures::segment_entry(&request, 0);
        entry.selection = selection;
        let mut mock = WorkflowMock::new::<RegenerateSegmentEfforts>(input);
        mock.activity("segment_activity_page", &request, &vec![entry]);
        mock.assert_failure_message("resource");
    }
}
