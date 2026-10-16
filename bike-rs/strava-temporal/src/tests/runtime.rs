//! Provider assertions on the neutral SDK workflow harness.
use bike_core::strava::events::StravaEvent;
use temporalio_common::protos::coresdk::workflow_commands::{
    workflow_command, StartChildWorkflowExecution, WorkflowCommand,
};
use workflow_test_support::WorkflowAssertions;

pub struct StravaAssertions;
pub type WorkflowMock = workflow_test_support::WorkflowMock<StravaAssertions>;

impl WorkflowAssertions for StravaAssertions {
    fn activity(name: &str, command: &WorkflowCommand) {
        let summary = command
            .user_metadata
            .as_ref()
            .unwrap()
            .summary
            .as_ref()
            .unwrap();
        assert_eq!(
            serde_json::from_slice::<String>(&summary.data).unwrap(),
            name.strip_prefix("strava_")
                .unwrap_or(name)
                .replace('_', " ")
        );
        let Some(workflow_command::Variant::ScheduleActivity(activity)) = &command.variant else {
            panic!("Expected activity {name}")
        };
        assert_eq!(activity.retry_policy.as_ref().unwrap().maximum_attempts, 12);
    }

    fn child(input: &serde_json::Value, command: &StartChildWorkflowExecution) {
        let input: StravaEvent = serde_json::from_value(input.clone()).unwrap();
        let attributes = command.search_attributes.as_ref().unwrap();
        for (key, expected) in [
            ("BikeCustomerId", input.bike_customer_id.to_string()),
            ("BikeUserId", input.bike_user_id.to_string()),
            ("StravaAthleteId", input.strava_athlete_id.to_string()),
            ("StravaEventId", input.strava_event_id.to_string()),
            ("CorrelationId", input.correlation_id.to_string()),
        ] {
            assert_eq!(
                serde_json::from_slice::<String>(&attributes.indexed_fields[key].data).unwrap(),
                expected
            );
        }
        if let Some(id) = input.strava_activity_id {
            assert_eq!(
                serde_json::from_slice::<String>(
                    &attributes.indexed_fields["StravaActivityId"].data
                )
                .unwrap(),
                id.to_string()
            );
        }
    }
}
