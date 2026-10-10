use api::{storage::AppStorage, tasks};
use axum::Router;
use bike_core::{
    auth::entities::users,
    background_jobs::{
        background_tasks,
        entities::{pipeline_runs, pipeline_tasks, task_attempts},
    },
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

pub const SYNTHETIC_KEY: &str = "internal-synthetic-test-credential-32chars";

pub async fn synthetic_platform_app() -> (Router, DatabaseConnection, users::Model) {
    init_test_metrics();
    // Keep an existing rider and records in the database to prove provisioning
    // allocates IDs and never adopts or edits a real owner's data.
    let db = platform_database().await;
    let now = Utc::now();
    users::ActiveModel {
        id: Set(1),
        pid: Set(Uuid::parse_str(USER_PID).unwrap()),
        email: Set("developer@bike.local".into()),
        api_key: Set("existing-key".into()),
        name: Set("Existing Rider".into()),
        disabled: Set(false),
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
    let user = bike_core::synthetics::ensure_scenario(&db).await.unwrap();
    let state = Arc::new(AppStorage {
        heatmaps: Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
        tasks: tasks::TaskQueue::new(db.clone()),
        feature_flags: bike_core::platform::feature_flags::FeatureFlagService::new(),
        session_service: tasks::create_session_service(db.clone()),
        db: db.clone(),
        uploads_dir: String::new(),
        local_admin_user_pid: None,
        synthetic_auth: Some(bike_core::synthetics::SyntheticAuth {
            key: SYNTHETIC_KEY.into(),
            user_pid: user.pid,
        }),
    });
    (api::app(state).await, db, user)
}

async fn build_platform_app(local_admin: bool) -> Router {
    ingestion_platform_app(local_admin, String::new()).await.0
}

pub async fn ingestion_platform_app(
    local_admin: bool,
    uploads_dir: String,
) -> (Router, DatabaseConnection) {
    init_test_metrics();
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
        heatmaps: Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
        tasks: tasks::TaskQueue::new(db.clone()),
        feature_flags: bike_core::platform::feature_flags::FeatureFlagService::new(),
        session_service: tasks::create_session_service(db.clone()),
        db: db.clone(),
        uploads_dir,
        local_admin_user_pid: local_admin.then_some(user_pid),
        synthetic_auth: None,
    });
    (api::app(state).await, db)
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
        activity_analytics::Entity,
        activity_training_analyses::Entity,
        user_preferences::Entity,
        analytics_user_states::Entity,
        segments::Entity,
        segment_efforts::Entity,
        segment_summaries::Entity,
        segment_user_summaries::Entity,
        bike_core::entities::synthetic_scenarios::Entity,
    );
    create_worker_tables(&db, &schema).await;
    create_ingestion_tables(&db, &schema).await;
    // This fixture has no external provider or shared database access.
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        "PRAGMA foreign_keys = ON",
    ))
    .await
    .unwrap();
    db
}

async fn create_ingestion_tables(db: &DatabaseConnection, schema: &Schema) {
    macro_rules! create_tables {
        ($($entity:expr),+ $(,)?) => {
            $(db.execute(&schema.create_table_from_entity($entity)).await.unwrap();)+
        };
    }
    create_tables!(
        activity_imports::Entity,
        bike_core::entities::activity_import_attempts::Entity,
        bike_core::entities::integration_events::Entity,
        bike_core::entities::activity_archive_import_jobs::Entity,
        activity_import_locks::Entity,
        activity_import_artifacts::Entity,
        bike_core::platform::feature_flags::entities::Entity,
    );
}

fn init_test_metrics() {
    static METRICS: Once = Once::new();
    METRICS.call_once(api::metrics::init_metrics);
}

async fn create_worker_tables(db: &DatabaseConnection, schema: &Schema) {
    for statement in [
        schema.create_table_from_entity(background_tasks::Entity),
        schema.create_table_from_entity(pipeline_runs::Entity),
        schema.create_table_from_entity(pipeline_tasks::Entity),
        schema.create_table_from_entity(task_attempts::Entity),
        schema.create_table_from_entity(
            bike_core::background_jobs::entities::processor_registry::Entity,
        ),
        schema.create_table_from_entity(
            bike_core::background_jobs::entities::pipeline_subjects::Entity,
        ),
        schema.create_table_from_entity(
            bike_core::background_jobs::entities::pipeline_outputs::Entity,
        ),
        schema.create_table_from_entity(bike_core::background_jobs::entities::work_units::Entity),
        schema
            .create_table_from_entity(bike_core::background_jobs::entities::task_anomalies::Entity),
        schema
            .create_table_from_entity(bike_core::background_jobs::entities::worker_batches::Entity),
        schema.create_table_from_entity(bike_core::background_jobs::entities::batch_tasks::Entity),
        schema.create_table_from_entity(bike_core::entities::strava_delivery_intents::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
}
