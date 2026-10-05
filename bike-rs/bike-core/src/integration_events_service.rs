use crate::entities::integration_events as integration_event_entity;
use crate::workflow_error::WorkflowError as AppError;
use sea_orm::{ConnectionTrait, DatabaseConnection};

pub const INTEGRATION_PROVIDER_STRAVA: &str = "strava";
pub const INTEGRATION_LEVEL_INFO: &str = "info";
pub const INTEGRATION_LEVEL_SUCCESS: &str = "success";
pub const INTEGRATION_LEVEL_WARNING: &str = "warning";
pub const INTEGRATION_LEVEL_ERROR: &str = "error";

pub use integration_event_entity::ListOptions as IntegrationEventListOptions;
pub use integration_event_entity::NewEvent as NewIntegrationEvent;

pub async fn record_event(
    db: &impl ConnectionTrait,
    event: NewIntegrationEvent,
) -> Result<integration_event_entity::Model, AppError> {
    integration_event_entity::Entity::record(db, event)
        .await
        .map_err(AppError::from)
}

pub async fn list_recent_events(
    db: &DatabaseConnection,
    options: IntegrationEventListOptions,
) -> Result<Vec<integration_event_entity::Model>, AppError> {
    integration_event_entity::Entity::recent(db, options)
        .await
        .map_err(AppError::from)
}
