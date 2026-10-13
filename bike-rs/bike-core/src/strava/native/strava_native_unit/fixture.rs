use super::super::*;
use crate::{auth::entities::users, entities::strava_events};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct Fixture {
    pub customer: users::Model,
    pub connection: strava_connections::Model,
    pub event: StravaEvent,
}

impl Fixture {
    pub fn new() -> Self {
        let now = DateTime::<Utc>::from_timestamp(1790856000, 0).unwrap();
        let customer = users::Model {
            id: 7,
            pid: Uuid::from_u128(7),
            email: "fixture@example.test".into(),
            password: None,
            api_key: "fixture".into(),
            name: "Fixture customer".into(),
            is_admin: Some(false),
            disabled: false,
            reset_token: None,
            reset_sent_at: None,
            email_verification_token: None,
            email_verification_sent_at: None,
            email_verified_at: Some(now),
            magic_link_token: None,
            magic_link_expiration: None,
            oauth_subject: None,
            oauth_provider: None,
            created_at: now,
            updated_at: now,
        };
        let connection = strava_connections::Model {
            id: 9,
            user_id: customer.id,
            athlete_id: 123,
            disconnect_requested_at: None,
            athlete_username: None,
            athlete_first_name: None,
            athlete_last_name: None,
            athlete_profile_medium_url: None,
            scopes: "activity:read_all".into(),
            access_token: "fixture-access".into(),
            refresh_token: "fixture-refresh".into(),
            expires_at: now,
            last_synced_activity_started_at: None,
            last_sync_status: "never".into(),
            last_sync_message: None,
            last_sync_started_at: None,
            last_sync_finished_at: None,
            last_sync_imported_count: 0,
            last_sync_duplicate_count: 0,
            last_sync_failed_count: 0,
            created_at: now,
            updated_at: now,
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
            id: 42,
            user_id: self.customer.id,
            activity_import_id: None,
            title: "Fixture ride".into(),
            sport: "ride".into(),
            source: "strava_sync".into(),
            source_correlation_id: self.event.strava_activity_id.map(|id| id.to_string()),
            original_filename: None,
            format: None,
            activity_type: "Ride".into(),
            started_at: self.connection.created_at,
            ended_at: None,
            distance_meters: None,
            moving_time_seconds: None,
            total_time_seconds: None,
            elevation_gain_meters: None,
            elevation_loss_meters: None,
            average_speed_mps: None,
            max_speed_mps: None,
            average_heart_rate_bpm: None,
            max_heart_rate_bpm: None,
            average_cadence_rpm: None,
            max_cadence_rpm: None,
            calories: None,
            estimated_ftp_watts: None,
            heart_rate_zones_json: None,
            derived_data_json: None,
            created_at: self.connection.created_at,
            updated_at: self.connection.created_at,
        }
    }
}
