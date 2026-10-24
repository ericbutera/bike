//! Strava connection management, provider operations, storage and durable work.
pub mod client;
mod connection;
pub use bike_core::entities::strava_connections::credentials;
pub mod events;
pub mod logging;
pub mod native;
pub mod oauth_state;
pub mod objects;
pub mod paths;
pub mod payload;
pub use bike_core::activity_parser::strava as provider_payload;
pub use bike_core::entities::strava_events::StravaWebhookEvent;
mod transport;

#[cfg(feature = "test-fixtures")]
#[path = "tests/fixtures.rs"]
pub mod test_fixtures;

#[cfg(test)]
pub(crate) mod fixtures;

pub use connection::{
    athlete_display_name, build_authorization_url, build_frontend_account_redirect, build_redirect_uri,
    create_authorization_url_for_user, exchange_code_for_connection, forget_local_connection, load_connection,
    parse_scope_list, scopes_allow_activity_import, verify_webhook_subscription, StravaWebhookChallengeResponse,
    StravaWebhookSubscriptionQuery, STRAVA_AUTHORIZE_URL, STRAVA_SYNC_STATUS_FAILED, STRAVA_SYNC_STATUS_NEVER,
    STRAVA_SYNC_STATUS_QUEUED, STRAVA_SYNC_STATUS_RUNNING,
};
pub(crate) use connection::{ensure_connection_scopes_allow_activity_import, ensure_fresh_access_token};
