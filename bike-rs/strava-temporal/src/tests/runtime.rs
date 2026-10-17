//! Provider assertions on the neutral SDK workflow harness.
use bike_core::platform::temporal as policy;
use bike_core::strava::events::StravaEvent;
use std::time::Duration;
use temporalio_common::protos::coresdk::workflow_commands::{
    workflow_command, StartChildWorkflowExecution, WorkflowCommand,
};
use workflow_test_support::WorkflowAssertions;

pub struct StravaAssertions;
pub type WorkflowMock = workflow_test_support::WorkflowMock<StravaAssertions>;

impl WorkflowAssertions for StravaAssertions {
    fn activity(name: &str, command: &WorkflowCommand) {
        let summary = command.user_metadata.as_ref().unwrap().summary.as_ref().unwrap();
        assert_eq!(
            serde_json::from_slice::<String>(&summary.data).unwrap(),
            name.strip_prefix("strava_").unwrap_or(name).replace('_', " ")
        );
        let Some(workflow_command::Variant::ScheduleActivity(activity)) = &command.variant else {
            panic!("Expected activity {name}")
        };
        assert_eq!(
            activity.retry_policy.as_ref().unwrap(),
            &policy::activity_retry_policy().into_raw()
        );
        assert_eq!(
            Duration::try_from(activity.start_to_close_timeout.unwrap()).unwrap(),
            policy::ACTIVITY_EXECUTION_TIMEOUT
        );
        assert_eq!(
            Duration::try_from(activity.heartbeat_timeout.unwrap()).unwrap(),
            policy::ACTIVITY_HEARTBEAT_TIMEOUT
        );
    }

    fn child(input: &serde_json::Value, command: &StartChildWorkflowExecution) {
        assert!(command.retry_policy.is_none());
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
                serde_json::from_slice::<String>(&attributes.indexed_fields["StravaActivityId"].data).unwrap(),
                id.to_string()
            );
        }
    }
}
