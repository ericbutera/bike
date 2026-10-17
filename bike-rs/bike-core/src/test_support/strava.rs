use crate::{
    entities::{strava_activity_states, strava_events},
    strava::{events::StravaEvent, native::ActivityStatus},
};

pub fn strava_activity_state() -> strava_activity_states::Model {
    let connection = super::connection();
    strava_activity_states::Model {
        athlete_id: connection.athlete_id,
        activity_id: 99133701,
        bike_activity_id: Some(super::activity().id),
        connection_generation: connection.created_at,
        event_time: connection.created_at.timestamp(),
        applied_hash: None,
        deleted: false,
        deleted_at: None,
        updated_at: connection.created_at,
        applied_at: connection.created_at,
    }
}

pub fn strava_event_record(event: &StravaEvent) -> strava_events::Model {
    let at = super::timestamp();
    strava_events::Model {
        id: event.strava_event_id.to_string(),
        event_key: event.strava_event_id.to_string(),
        correlation_id: event.correlation_id.to_string(),
        parent_event_id: event.parent_event_id.map(|id| id.to_string()),
        user_id: event.bike_user_id,
        customer_id: event.bike_customer_id,
        athlete_id: event.strava_athlete_id,
        activity_id: event.strava_activity_id,
        workflow_id: event.workflow_id(),
        event: serde_json::to_value(event).expect("valid event fixture"),
        status: ActivityStatus::Pending,
        mutation: None,
        accepted_at: at,
        submitted_at: None,
        next_attempt_at: at,
        handoff_error: None,
    }
}
