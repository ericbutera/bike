//! Strava connection management, provider operations, storage and durable work.
pub mod activity;
pub(crate) mod archive_json;
pub mod client;
mod connection;
pub mod credentials;
pub mod objects;
pub mod paths;
pub mod provider_payload;
pub mod retention;
mod transport;
pub mod workflow;

#[cfg(test)]
pub(crate) mod fixtures;

pub use connection::{
    athlete_display_name, build_authorization_url, build_frontend_account_redirect,
    build_redirect_uri, create_authorization_url_for_user, create_state_token,
    exchange_code_for_connection, forget_local_connection, load_connection, parse_scope_list,
    resolve_connection_sync_state, scopes_allow_activity_import, verify_state_token,
    verify_webhook_subscription, ResolvedStravaConnectionSyncState, StravaWebhookChallengeResponse,
    StravaWebhookEvent, StravaWebhookSubscriptionQuery, VerifiedStravaState, STRAVA_AUTHORIZE_URL,
    STRAVA_SYNC_STATUS_FAILED, STRAVA_SYNC_STATUS_NEVER, STRAVA_SYNC_STATUS_QUEUED,
    STRAVA_SYNC_STATUS_RUNNING,
};
pub(crate) use connection::{
    build_activity_upload, ensure_connection_scopes_allow_activity_import,
    ensure_fresh_access_token,
};
