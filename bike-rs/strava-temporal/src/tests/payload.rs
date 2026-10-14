use super::{fixture::Fixture, runtime::WorkflowMock};
use crate::{activities::checked_output, workflows::UpsertActivity};
use bike_core::strava::{
    events::StravaOperation,
    payload::{validate_temporal, MAX_TEMPORAL_JSON_BYTES},
};

#[test]
fn payload_budget_accepts_the_boundary_and_counts_utf8_and_json_escaping() {
    validate_temporal(&"a".repeat(MAX_TEMPORAL_JSON_BYTES - 2)).unwrap();
    for oversized in [
        "a".repeat(MAX_TEMPORAL_JSON_BYTES - 1),
        "é".repeat(MAX_TEMPORAL_JSON_BYTES / 2),
        "\n".repeat(MAX_TEMPORAL_JSON_BYTES / 2),
    ] {
        let error = validate_temporal(&oversized).unwrap_err();
        assert_eq!(error.status.as_u16(), 413);
        assert!(error.message.contains("pass a reference"));
        assert!(!error.message.contains(&oversized));
    }
}

#[test]
fn growing_activity_dependencies_are_rejected_before_encoding_an_activity_result() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let mut change = fixture.change(bike_core::strava::native::ActivityStatus::Applied);
    change.segment_ids = vec![i32::MAX; 30_000];
    assert_eq!(checked_output(change).unwrap_err().status.as_u16(), 413);
    checked_output(fixture.change(bike_core::strava::native::ActivityStatus::Applied)).unwrap();
}

#[test]
fn oversized_workflow_input_fails_before_scheduling_provider_work() {
    let mut event = Fixture::new(StravaOperation::Upsert).event;
    event.request_id = Some("a".repeat(MAX_TEMPORAL_JSON_BYTES));
    let workflow = WorkflowMock::new::<UpsertActivity>(event);
    workflow.assert_failure_message("limit is 262144");
}

#[test]
fn pre_budget_history_keeps_its_recorded_activity_before_failure() {
    let mut event = Fixture::new(StravaOperation::Upsert).event;
    event.request_id = Some("a".repeat(MAX_TEMPORAL_JSON_BYTES));
    let mut workflow = WorkflowMock::with_replay::<UpsertActivity>(event, true);
    workflow.fail_activity(
        crate::activities::StravaActivities::fetch_activity.name(),
        "Historical provider failure",
    );
    workflow.assert_failure_message("Historical provider failure");
}
