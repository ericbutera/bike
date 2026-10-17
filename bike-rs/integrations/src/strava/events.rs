//! Strava request acceptance and the stored event contract.
pub use bike_core::entities::strava_events::{webhook_operation, SourceVersion, StravaEvent, StravaOperation};
mod intake;
pub use intake::{accept_webhook, request_backfill, request_disconnect, request_sync};
