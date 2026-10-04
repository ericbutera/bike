use api::{storage::AppStorage, tasks};
use axum::Router;
use bike_core::{
    auth::entities::users,
    background_jobs::background_tasks,
    entities::{
        activities, activity_analytics, activity_import_artifacts, activity_import_locks,
        activity_imports, activity_training_analyses, analytics_user_states, segment_efforts,
        segment_summaries, segment_user_summaries, segments, user_preferences,
    },
};
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, Database, DatabaseConnection, Schema, Set, Statement,
};
use std::sync::{Arc, Once};
use uuid::Uuid;

pub const USER_PID: &str = "00000000-0000-4000-8000-000000000001";

pub async fn platform_app() -> Router {
    build_platform_app(true).await
}

pub async fn protected_platform_app() -> Router {
    build_platform_app(false).await
}

async fn build_platform_app(local_admin: bool) -> Router {
    static METRICS: Once = Once::new();
    METRICS.call_once(api::metrics::init_metrics);
    let db = platform_database().await;
    let user_pid = Uuid::parse_str(USER_PID).unwrap();
    let now = Utc::now();
    users::ActiveModel {
        id: Set(1),
        pid: Set(user_pid),
        email: Set("developer@bike.local".into()),
        api_key: Set("platform-test-key".into()),
        name: Set("Fixture Rider".into()),
        is_admin: Set(Some(true)),
        disabled: Set(false),
        email_verified_at: Set(Some(now)),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    db.execute_unprepared(include_str!("../fixtures/platform/read-models.sql"))
        .await
        .unwrap();
    let state = Arc::new(AppStorage {
        tasks: tasks::TaskQueue::new(db.clone()),
        feature_flags: bike_core::platform::feature_flags::FeatureFlagService::new(),
        session_service: tasks::create_session_service(db.clone()),
        db,
        uploads_dir: String::new(),
        local_admin_user_pid: local_admin.then_some(user_pid),
    });
    api::app(state).await
}

async fn platform_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    macro_rules! create_tables {
        ($($entity:expr),+ $(,)?) => {
            $(db.execute(&schema.create_table_from_entity($entity)).await.unwrap();)+
        };
    }
    create_tables!(
        users::Entity,
        activities::Entity,
        activity_imports::Entity,
        activity_import_locks::Entity,
        activity_import_artifacts::Entity,
        activity_analytics::Entity,
        activity_training_analyses::Entity,
        user_preferences::Entity,
        analytics_user_states::Entity,
        background_tasks::Entity,
        segments::Entity,
        segment_efforts::Entity,
        segment_summaries::Entity,
        segment_user_summaries::Entity,
    );
    // No worker or external provider runs against this isolated fixture.
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        "PRAGMA foreign_keys = ON",
    ))
    .await
    .unwrap();
    db
}
