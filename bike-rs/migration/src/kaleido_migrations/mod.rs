//! Bike-owned copies of Kaleido 0.10.1 migration records.
//!
//! The migration source files are carried forward verbatim so an existing
//! database keeps the same migration names and schema history.
pub use sea_orm_migration::prelude::*;
mod m20220101_000001_users;
mod m20220101_000002_refresh_tokens;
mod m20260107_120000_create_cooldowns;
mod m20260122_120000_background_tasks;
mod m20260124_120500_api_clients;
mod m20260125_212110_create_feature_flags;
mod m20260210_000001_auth_events;
mod m20260312_000000_background_tasks_result;
mod m20260724_000001_remove_oauth_feature_flag;
mod m20260724_000002_rename_oauth_subject_column;
mod m20260928_000001_add_user_disabled;

pub fn bike_platform_migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20220101_000001_users::Migration),
        Box::new(m20220101_000002_refresh_tokens::Migration),
        Box::new(m20260107_120000_create_cooldowns::Migration),
        Box::new(m20260122_120000_background_tasks::Migration),
        Box::new(m20260124_120500_api_clients::Migration),
        Box::new(m20260125_212110_create_feature_flags::Migration),
        Box::new(m20260210_000001_auth_events::Migration),
        Box::new(m20260312_000000_background_tasks_result::Migration),
        Box::new(m20260724_000001_remove_oauth_feature_flag::Migration),
        Box::new(m20260724_000002_rename_oauth_subject_column::Migration),
        Box::new(m20260928_000001_add_user_disabled::Migration),
    ]
}
