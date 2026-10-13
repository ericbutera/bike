//! Strava operations coordinated through the existing durable worker.
mod action;
mod activity;
mod child;
mod collection;
mod context;
mod execution;
pub(crate) mod import_objects;
mod inbox;
pub(crate) mod intents;
pub(crate) mod rebuild;
mod revocation;
mod sources;
mod stage;
pub mod steps;

#[cfg(test)]
mod tests;

pub use action::{Action, Execution, WorkflowTask};
use context::Run;
pub use context::Source;
pub use execution::{execute, execute_for};
pub use inbox::{
    accept_webhook, active_user_collection, queue_disconnect, queue_reconciliation, queue_sync,
    queue_user_sync, resume_at, wait_for_children,
};

/// New-user onboarding must never automatically request more than 15 days.
pub const INITIAL_BACKFILL_DAYS: i64 = 15;

pub fn provider_work_enabled() -> bool {
    std::env::var("STRAVA_PROVIDER_WORK_ENABLED").as_deref() != Ok("false")
}

pub fn graph(action: &str) -> Result<Vec<&'static str>, crate::workflow_error::WorkflowError> {
    Ok(action.parse::<Action>()?.stages().to_vec())
}
