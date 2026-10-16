use super::super::*;
use crate::{auth::entities::users, entities::strava_events};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct Fixture {
    pub customer: users::Model,
    pub connection: strava_connections::Model,
    pub event: StravaEvent,
}

pub(super) fn audit(fixture: &Fixture) -> crate::entities::integration_events::Model {
    crate::entities::integration_events::Model {
        id: 1,
        user_id: Some(fixture.customer.id),
        provider: "strava".into(),
        event_type: "strava.event.accepted".into(),
        level: "info".into(),
        message: "Fixture acceptance".into(),
        connection_id: Some(fixture.connection.id),
        payload: Some(serde_json::json!({"strava_event_id": fixture.event.strava_event_id})),
        created_at: fixture.connection.created_at,
    }
}

impl Fixture {
    pub fn new() -> Self {
        let now = DateTime::<Utc>::from_timestamp(1790856000, 0).unwrap();
        let customer = users::Model {
            pid: Uuid::from_u128(7),
            api_key: "fixture".into(),
            email_verified_at: Some(now),
            created_at: now,
            updated_at: now,
            ..crate::test_support::customer()
        };
        let connection = strava_connections::Model {
            user_id: customer.id,
            athlete_username: None,
            athlete_first_name: None,
            athlete_last_name: None,
            athlete_profile_medium_url: None,
            scopes: "activity:read_all".into(),
            expires_at: now,
            last_synced_activity_started_at: None,
            created_at: now,
            updated_at: now,
            ..crate::test_support::connection()
        };
        let mut event = StravaEvent::new(
            &customer,
            &connection,
            super::super::super::events::StravaOperation::Delete,
        );
        event.strava_event_id = Uuid::from_u128(1);
        event.correlation_id = event.strava_event_id;
        event.strava_activity_id = Some(99133701);
        event.event_time = now.timestamp();
        Self {
            customer,
            connection,
            event,
        }
    }

    pub fn change(&self, status: ActivityStatus) -> ActivityChange {
        ActivityChange::unchanged(self.event.clone(), status)
    }

    pub fn row(&self, status: ActivityStatus) -> strava_events::Model {
        strava_events::Model {
            id: self.event.strava_event_id.to_string(),
            event_key: "fixture-event".into(),
            correlation_id: self.event.correlation_id.to_string(),
            parent_event_id: None,
            user_id: self.customer.id,
            customer_id: self.customer.pid,
            athlete_id: self.connection.athlete_id,
            activity_id: self.event.strava_activity_id,
            workflow_id: self.event.workflow_id(),
            event: serde_json::to_value(&self.event).unwrap(),
            status,
            mutation: None,
            accepted_at: self.connection.created_at,
            submitted_at: None,
            next_attempt_at: self.connection.created_at,
            handoff_error: None,
        }
    }

    pub fn activity(&self) -> activities::Model {
        activities::Model {
            user_id: self.customer.id,
            source: "strava_sync".into(),
            source_correlation_id: self.event.strava_activity_id.map(|id| id.to_string()),
            activity_type: "Ride".into(),
            started_at: self.connection.created_at,
            created_at: self.connection.created_at,
            updated_at: self.connection.created_at,
            ..crate::test_support::activity()
        }
    }
}
