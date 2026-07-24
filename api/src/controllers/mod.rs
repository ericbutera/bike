use crate::config::Config;
pub mod activities;
pub mod activity_imports;
pub mod admin;
pub mod fitness;
pub mod garmin_iq;
pub mod integration_events;
pub mod reports;
pub mod segments;
pub mod strava;
pub mod training_goals;
pub mod user_preferences;

use crate::storage::AppStorage;
use axum::{extract::DefaultBodyLimit, routing::get, Json, Router};
use kaleido::auth;
use kaleido::auth::AdminUserContext;
use kaleido::background_jobs;
use kaleido::glass::feature_flags;
use kaleido::glass::metrics_controller;
use serde_json::json;
use std::sync::Arc;

pub fn routes() -> Router<Arc<AppStorage>> {
    Router::new()
        .nest("/api", auth::routes())
        .nest("/api/oauth", auth::oauth_routes())
        .nest("/api/admin", admin::routes())
        .nest(
            "/api/admin/integration-events",
            integration_events::admin_routes(),
        )
        .nest("/api/admin/feature-flags", feature_flags::admin_routes())
        .nest("/api/feature-flags", feature_flags::public_routes())
        .nest(
            "/api",
            background_jobs::admin::api_routes::<AppStorage, AdminUserContext<AppStorage>>(),
        )
        .nest("/api/admin/users", auth::admin_routes())
        .route(
            "/api/activities",
            axum::routing::get(activities::list_activities),
        )
        .route(
            "/api/activities/:id",
            axum::routing::get(activities::get_activity)
                .patch(activities::update_activity)
                .delete(activities::delete_activity),
        )
        .route(
            "/api/activities/:id/regenerate",
            axum::routing::post(activities::regenerate_activity),
        )
        .route(
            "/api/fitness",
            axum::routing::get(fitness::get_fitness_freshness),
        )
        .route(
            "/api/training/xc-progress",
            axum::routing::get(training_goals::get_xc_goal_progress),
        )
        .route(
            "/api/training/dh-progress",
            axum::routing::get(training_goals::get_dh_goal_progress),
        )
        .route(
            "/api/training/reports",
            axum::routing::get(reports::get_training_reports),
        )
        .route(
            "/api/segments",
            axum::routing::get(segments::list_segments)
                .post(segments::import_segment)
                .layer(DefaultBodyLimit::max(Config::get().max_upload_bytes)),
        )
        .route(
            "/api/segments/from-activity",
            axum::routing::post(segments::create_segment_from_activity),
        )
        .route(
            "/api/segments/:id/from-activity",
            axum::routing::put(segments::update_segment_from_activity),
        )
        .route(
            "/api/segments/:id",
            axum::routing::get(segments::get_segment)
                .put(segments::update_segment)
                .delete(segments::delete_segment),
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
        .route(
            "/api/activity-imports/archive-url",
            axum::routing::post(activity_imports::import_activity_archive_from_url),
        )
        .route(
            "/api/activity-imports/archive-jobs",
            axum::routing::get(activity_imports::list_activity_archive_import_jobs),
        )
        .route(
            "/api/activity-imports/processing-state",
            axum::routing::get(activity_imports::get_activity_processing_state),
        )
        .route(
            "/api/activity-imports/archive-jobs/:id",
            axum::routing::get(activity_imports::get_activity_archive_import_job),
        )
        .nest("/api/integration-events", integration_events::routes())
        .nest("/api/garmin-iq", garmin_iq::routes())
        .nest("/api/strava", strava::routes())
        .nest(
            "/api/admin/metrics",
            metrics_controller::admin_routes::<AppStorage>(),
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
