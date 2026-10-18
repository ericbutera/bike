use crate::auth::{
    entities::{auth_events, users},
    error::AuthError,
    extractors::{AdminUserContext, AuthStorage},
};
use crate::platform::data::pagination::{PaginatedResponse, PaginationParams};
use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Serialize;
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct AdminUserAuditEventResponse {
    pub id: i64,
    pub event_type: String,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub reason: Option<String>,
    pub occurred_at: String,
}

impl From<auth_events::Model> for AdminUserAuditEventResponse {
    fn from(event: auth_events::Model) -> Self {
        Self {
            id: event.id,
            event_type: event.event_type,
            ip: event.ip,
            user_agent: event.user_agent,
            reason: event.reason,
            occurred_at: event.ts.to_rfc3339(),
        }
    }
}

pub type AdminUserAuditEventsResponse = PaginatedResponse<AdminUserAuditEventResponse>;

#[utoipa::path(
    get,
    path = "/admin/users/{id}/audit-events",
    params(("id" = i32, Path, description = "User ID"), PaginationParams),
    responses(
        (status = 200, description = "User audit events, newest first", body = AdminUserAuditEventsResponse),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "User not found"),
    ),
    tag = "admin",
    security(("bearer_auth" = []))
)]
pub async fn list_user_audit_events<S>(
    _admin: AdminUserContext<S>,
    State(state): State<Arc<S>>,
    Path(id): Path<i32>,
    Query(params): Query<PaginationParams>,
) -> Result<Json<AdminUserAuditEventsResponse>, AuthError>
where
    S: AuthStorage + 'static,
{
    users::Model::find_by_id(state.db(), id).await?;
    let (page, per_page) = params.normalized();
    let (events, total) = auth_events::Model::for_user_page(state.db(), id, page as u64, per_page as u64).await?;
    let events = events.into_iter().map(AdminUserAuditEventResponse::from).collect();
    Ok(Json(PaginatedResponse::new(events, page, per_page, total as i64)))
}
