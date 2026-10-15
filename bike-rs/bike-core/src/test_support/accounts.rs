//! Shared account and provider-connection fixtures.
use super::timestamp;
use crate::{auth::entities::users, entities::strava_connections};

pub fn customer() -> users::Model {
    let at = timestamp();
    users::Model {
        id: 7,
        pid: uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap(),
        email: "fixture@example.test".into(),
        password: None,
        api_key: "fixture-key".into(),
        name: "Fixture customer".into(),
        is_admin: Some(false),
        disabled: false,
        reset_token: None,
        reset_sent_at: None,
        email_verification_token: None,
        email_verification_sent_at: None,
        email_verified_at: Some(at),
        magic_link_token: None,
        magic_link_expiration: None,
        oauth_subject: None,
        oauth_provider: None,
        created_at: at,
        updated_at: at,
    }
}

pub fn connection() -> strava_connections::Model {
    let at = customer().created_at;
    strava_connections::Model {
        id: 9,
        user_id: 7,
        athlete_id: 123,
        disconnect_requested_at: None,
        athlete_username: Some("fixture-rider".into()),
        athlete_first_name: Some("Fixture".into()),
        athlete_last_name: Some("Rider".into()),
        athlete_profile_medium_url: Some("https://example.test/rider.jpg".into()),
        scopes: "read,activity:read_all".into(),
        access_token: "fixture-access".into(),
        refresh_token: "fixture-refresh".into(),
        expires_at: at,
        last_synced_activity_started_at: Some(at),
        last_sync_status: "never".into(),
        last_sync_message: None,
        last_sync_started_at: None,
        last_sync_finished_at: None,
        last_sync_imported_count: 0,
        last_sync_duplicate_count: 0,
        last_sync_failed_count: 0,
        created_at: at,
        updated_at: at,
    }
}
