use crate::app_error::AppError;
use crate::entities::integration_events as integration_event_entity;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseBackend, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set,
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
    pub activity_id: Option<i32>,
    pub import_id: Option<i32>,
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

    if let Some(activity_id) = options.activity_id {
        query = query.filter(json_payload_int_filter(
            db.get_database_backend(),
            "activity_id",
            activity_id,
        ));
    }

    if let Some(import_id) = options.import_id {
        query = query.filter(json_payload_int_filter(
            db.get_database_backend(),
            "import_id",
            import_id,
        ));
    }

    query
        .limit(options.limit.max(1))
        .all(db)
        .await
        .map_err(AppError::from)
}

fn json_payload_int_filter(
    backend: DatabaseBackend,
    key: &str,
    value: i32,
) -> sea_orm::sea_query::SimpleExpr {
    match backend {
        DatabaseBackend::Postgres => {
            Expr::cust(format!("payload->>'{key}' = '{value}'")).into()
        }
        DatabaseBackend::Sqlite => Expr::cust(format!(
            "(json_extract(payload, '$.{key}') = {value} OR json_extract(payload, '$.{key}') = '{value}')"
        ))
        .into(),
        DatabaseBackend::MySql => Expr::cust(format!(
            "JSON_UNQUOTE(JSON_EXTRACT(payload, '$.{key}')) = '{value}'"
        ))
        .into(),
        _ => Expr::cust(format!("payload->>'{key}' = '{value}'")).into(),
    }
}
