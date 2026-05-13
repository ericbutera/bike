use crate::app_error::AppError;
use crate::entities::integration_events as integration_event_entity;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::Value;

pub const INTEGRATION_PROVIDER_STRAVA: &str = "strava";
pub const INTEGRATION_LEVEL_INFO: &str = "info";
pub const INTEGRATION_LEVEL_SUCCESS: &str = "success";
pub const INTEGRATION_LEVEL_WARNING: &str = "warning";
pub const INTEGRATION_LEVEL_ERROR: &str = "error";

#[derive(Debug, Clone)]
pub struct NewIntegrationEvent {
    pub user_id: Option<i32>,
    pub provider: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    pub connection_id: Option<i32>,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct IntegrationEventListOptions {
    pub provider: Option<String>,
    pub user_id: Option<i32>,
    pub limit: u64,
}

pub async fn record_event(
    db: &DatabaseConnection,
    event: NewIntegrationEvent,
) -> Result<integration_event_entity::Model, AppError> {
    integration_event_entity::ActiveModel {
        user_id: Set(event.user_id),
        provider: Set(event.provider),
        event_type: Set(event.event_type),
        level: Set(event.level),
        message: Set(event.message),
        connection_id: Set(event.connection_id),
        payload: Set(event.payload),
        ..Default::default()
    }
    .insert(db)
    .await
    .map_err(AppError::from)
}

pub async fn list_recent_events(
    db: &DatabaseConnection,
    options: IntegrationEventListOptions,
) -> Result<Vec<integration_event_entity::Model>, AppError> {
    let mut query = integration_event_entity::Entity::find()
        .order_by_desc(integration_event_entity::Column::CreatedAt);

    if let Some(provider) = options.provider {
        query = query.filter(integration_event_entity::Column::Provider.eq(provider));
    }

    if let Some(user_id) = options.user_id {
        query = query.filter(integration_event_entity::Column::UserId.eq(user_id));
    }

    query
        .limit(options.limit.max(1))
        .all(db)
        .await
        .map_err(AppError::from)
}