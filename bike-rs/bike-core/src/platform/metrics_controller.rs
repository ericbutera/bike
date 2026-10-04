use crate::auth::extractors::{AdminUserContext, AuthStorage};
use crate::platform::system_metrics::SystemMetrics;
use axum::{extract::State, routing::get, Json, Router};
use sea_orm::DatabaseConnection;
use std::sync::Arc;

/// Trait required to mount the glass-managed metrics route.
/// Implement this on your `AppStorage` alongside `AuthStorage`.
pub trait MetricsStorage: Send + Sync + 'static {
    fn db(&self) -> &DatabaseConnection;
}

/// Returns axum routes for the glass-managed metrics endpoint.
///
/// Mount at `/api/admin/metrics`:
/// ```ignore
/// .nest("/api/admin/metrics", glass::metrics_controller::admin_routes::<AppStorage>())
/// ```
pub fn admin_routes<S>() -> Router<Arc<S>>
where
    S: MetricsStorage + AuthStorage,
{
    Router::new().route("/", get(get_metrics::<S>))
}

/// Get glass-managed system metrics (auth, background tasks, etc.)
///
/// Glass automatically expands this response as new subsystems are added.
#[utoipa::path(
    get,
    path = "/admin/metrics",
    operation_id = "admin_get_metrics",
    responses(
        (status = 200, description = "Glass system metrics", body = SystemMetrics),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
    ),
    security(("bearer_auth" = [])),
    tag = "admin",
)]
pub async fn get_metrics<S>(
    _admin: AdminUserContext<S>,
    State(state): State<Arc<S>>,
) -> Json<SystemMetrics>
where
    S: MetricsStorage + AuthStorage,
{
    Json(SystemMetrics::collect(MetricsStorage::db(&*state)).await)
}
