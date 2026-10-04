pub mod aggregator;
pub mod api_metrics;
pub mod auth_metrics;
pub mod background_task_metrics;
pub mod cooldown;
pub mod data;
pub mod email;
pub mod error;
pub mod feature_flags;
pub mod metrics_controller;
pub mod openapi;
pub mod system_metrics;

pub use error::PlatformError;
pub use openapi::SecurityAddon;
