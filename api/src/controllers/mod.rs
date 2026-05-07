use crate::config::Config;
pub mod activities;
pub mod activity_imports;
pub mod fitness;
pub mod segments;
pub mod user_preferences;

use crate::storage::AppStorage;
use kaleido::auth;
use kaleido::auth::AdminUserContext;
use kaleido::background_jobs;
use axum::{extract::DefaultBodyLimit, routing::get, Json, Router};
use kaleido::glass::feature_flags;
use serde_json::json;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .nest("/api", auth::routes())
        .nest("/api/oauth", auth::oauth_routes())
        .nest("/api/admin/feature-flags", feature_flags::admin_routes())
        .nest("/api/feature-flags", feature_flags::public_routes())
        .nest(
            "/api/admin/tasks",
            background_jobs::admin::admin_routes::<AppStorage, AdminUserContext<AppStorage>>(),
        )
        .nest("/api/admin/users", auth::admin_routes())
        .route(
            "/api/activities",
            axum::routing::get(activities::list_activities),
        )
        .route(
            "/api/activities/:id",
            axum::routing::get(activities::get_activity).delete(activities::delete_activity),
        )
        .route(
            "/api/activities/:id/regenerate",
            axum::routing::post(activities::regenerate_activity),
        )
        .route("/api/fitness", axum::routing::get(fitness::get_fitness_freshness))
        .route(
            "/api/segments",
            axum::routing::get(segments::list_segments)
                .post(segments::import_segment)
                .layer(DefaultBodyLimit::max(Config::get().max_upload_bytes)),
        )
        .route(
            "/api/segments/:id",
            axum::routing::get(segments::get_segment),
        )
        .route(
            "/api/preferences",
            axum::routing::get(user_preferences::get_preferences)
                .put(user_preferences::update_preferences),
        )
        .route(
            "/api/activity-imports",
            axum::routing::get(activity_imports::list_activity_imports)
                .post(activity_imports::upload_activity_import)
                .layer(DefaultBodyLimit::max(Config::get().max_upload_bytes)),
        )
        .route("/api/health", get(health))
        .route("/", get(root))
}

async fn root() -> Json<serde_json::Value> {
    Json(json!({ "service": "api", "status": "ok" }))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "healthy" }))
}
