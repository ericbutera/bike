use crate::{
    auth::entities::users,
    entities::{activities, strava_connections, strava_events},
};
use chrono::{DateTime, Utc};
use strava_events::{
    ActivityChange, ActivityStatus, NormalizedActivity, StravaEvent, StravaOperation, StravaWebhookEvent,
};
use uuid::Uuid;

pub struct Fixture {
    pub customer: users::Model,
    pub connection: strava_connections::Model,
    pub event: StravaEvent,
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

pub fn audit(fixture: &Fixture) -> crate::entities::integration_events::Model {
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
    pub fn normalized_activity(&self) -> NormalizedActivity {
        let source = crate::object_store::ObjectRef {
            path: "strava/fixture/normalized.json".into(),
            sha256: "a".repeat(64),
            bytes: 123,
        };
        NormalizedActivity {
            event: self.event.clone(),
            parsed: Some(crate::object_store::ObjectRef {
                path: "strava/fixture/parsed.json".into(),
                ..source.clone()
            }),
            source,
        }
    }
    pub fn activity_state(&self) -> crate::entities::strava_activity_states::Model {
        crate::entities::strava_activity_states::Model {
            athlete_id: self.event.strava_athlete_id,
            activity_id: self.event.strava_activity_id.unwrap(),
            bike_activity_id: Some(self.activity().id),
            connection_generation: self.event.connection_generation,
            event_time: self.event.event_time,
            updated_at: self.connection.created_at,
            applied_at: self.connection.created_at,
            ..crate::test_support::strava_activity_state()
        }
    }

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
        let mut event = StravaEvent::new(&customer, &connection, StravaOperation::Delete);
        event.strava_event_id = Uuid::from_u128(1);
        event.correlation_id = event.strava_event_id;
        event.strava_activity_id = Some(99133701);
        event.event_time = now.timestamp();
        event.webhook = Some(StravaWebhookEvent {
            aspect_type: "delete".into(),
            event_time: now.timestamp(),
            object_id: event.activity_id().unwrap(),
            object_type: "activity".into(),
            owner_id: event.strava_athlete_id,
            subscription_id: 1,
            updates: None,
            additional_fields: Default::default(),
        });
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
            event_key: "fixture-event".into(),
            status,
            accepted_at: self.connection.created_at,
            next_attempt_at: self.connection.created_at,
            ..crate::test_support::strava_event_record(&self.event)
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
