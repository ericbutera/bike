use crate::app_error::{ApiErrorResponse, AppError};
use crate::entities::integration_events;
use crate::integration_events as integration_event_service;
use crate::storage::AppStorage;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use kaleido::auth::{AdminUserContext, UserContext};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

const USER_HISTORY_LIMIT: u64 = 25;
const DEFAULT_ADMIN_LIMIT: u64 = 100;
const MAX_ADMIN_LIMIT: u64 = 200;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new().route("/strava", get(list_strava_history))
}

pub fn admin_routes() -> Router<Arc<AppStorage>> {
    Router::new().route("/", get(list_admin_integration_events))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IntegrationEventResponse {
    pub id: i32,
    pub user_id: Option<i32>,
    pub provider: String,
    pub event_type: String,
    pub level: String,
    pub message: String,
    pub connection_id: Option<i32>,
    pub payload: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct IntegrationEventsQuery {
    pub provider: Option<String>,
    pub user_id: Option<i32>,
    pub limit: Option<u64>,
}

impl IntegrationEventResponse {
    fn from_model(model: integration_events::Model) -> Self {
        Self {
            id: model.id,
            user_id: model.user_id,
            provider: model.provider,
            event_type: model.event_type,
            level: model.level,
            message: model.message,
            connection_id: model.connection_id,
            payload: model.payload,
            created_at: model.created_at,
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/integration-events/strava",
    responses(
        (status = 200, description = "Recent Strava integration history for the authenticated user", body = [IntegrationEventResponse]),
        (status = 401, description = "Not authenticated"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "strava",
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn list_strava_history(
    UserContext { user, .. }: UserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
) -> Result<Json<Vec<IntegrationEventResponse>>, AppError> {
    let events = integration_event_service::list_recent_events(
        &state.db,
        integration_event_service::IntegrationEventListOptions {
            provider: Some(integration_event_service::INTEGRATION_PROVIDER_STRAVA.to_string()),
            user_id: Some(user.id),
            limit: USER_HISTORY_LIMIT,
        },
    )
    .await?;

    Ok(Json(
        events
            .into_iter()
            .map(IntegrationEventResponse::from_model)
            .collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/api/admin/integration-events",
    params(
        ("provider" = Option<String>, Query, description = "Optional integration provider filter"),
        ("user_id" = Option<i32>, Query, description = "Optional Bike user id filter"),
        ("limit" = Option<u64>, Query, description = "Maximum number of rows to return"),
    ),
    responses(
        (status = 200, description = "Recent integration events for administrators", body = [IntegrationEventResponse]),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error", body = ApiErrorResponse),
    ),
    tag = "admin",
    security(("bearer_auth" = [])),
)]
pub async fn list_admin_integration_events(
    _admin: AdminUserContext<AppStorage>,
    State(state): State<Arc<AppStorage>>,
    Query(query): Query<IntegrationEventsQuery>,
) -> Result<Json<Vec<IntegrationEventResponse>>, AppError> {
    let events = integration_event_service::list_recent_events(
        &state.db,
        integration_event_service::IntegrationEventListOptions {
            provider: query.provider,
            user_id: query.user_id,
            limit: query
                .limit
                .unwrap_or(DEFAULT_ADMIN_LIMIT)
                .clamp(1, MAX_ADMIN_LIMIT),
        },
    )
    .await?;

    Ok(Json(
        events
            .into_iter()
            .map(IntegrationEventResponse::from_model)
            .collect(),
    ))
}
