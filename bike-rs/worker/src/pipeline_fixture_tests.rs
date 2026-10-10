use bike_core::{background_jobs::entities::*, entities::*};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Schema};

pub async fn database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(db.get_database_backend());
    for statement in [
        schema.create_table_from_entity(background_tasks::Entity),
        schema.create_table_from_entity(pipeline_runs::Entity),
        schema.create_table_from_entity(pipeline_tasks::Entity),
        schema.create_table_from_entity(task_attempts::Entity),
        schema.create_table_from_entity(pipeline_outputs::Entity),
        schema.create_table_from_entity(pipeline_subjects::Entity),
        schema.create_table_from_entity(processor_registry::Entity),
        schema.create_table_from_entity(work_units::Entity),
        schema.create_table_from_entity(task_anomalies::Entity),
        schema.create_table_from_entity(worker_batches::Entity),
        schema.create_table_from_entity(batch_tasks::Entity),
        schema.create_table_from_entity(activity_import_locks::Entity),
        schema.create_table_from_entity(activities::Entity),
        schema.create_table_from_entity(activity_imports::Entity),
        schema.create_table_from_entity(activity_import_attempts::Entity),
        schema.create_table_from_entity(activity_archive_import_jobs::Entity),
        schema.create_table_from_entity(activity_analytics::Entity),
        schema.create_table_from_entity(segment_efforts::Entity),
        schema.create_table_from_entity(segments::Entity),
        schema.create_table_from_entity(segment_summaries::Entity),
        schema.create_table_from_entity(segment_user_summaries::Entity),
        schema.create_table_from_entity(strava_connections::Entity),
        schema.create_table_from_entity(strava_delivery_intents::Entity),
        schema.create_table_from_entity(strava_gateway_watermarks::Entity),
        schema.create_table_from_entity(strava_gateway_revocations::Entity),
        schema.create_table_from_entity(integration_events::Entity),
        schema.create_table_from_entity(bike_core::platform::feature_flags::entities::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    // These receipt tables use migration-owned database clock defaults. Entity
    // schema generation does not include those defaults.
    db.execute_unprepared("CREATE TABLE strava_gateway_bindings (athlete_id BIGINT PRIMARY KEY, user_id INTEGER NOT NULL, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, active_delivery_id TEXT, lease_until TEXT)").await.unwrap();
    db.execute_unprepared("CREATE TABLE strava_gateway_receipts (delivery_id TEXT PRIMARY KEY, athlete_id BIGINT NOT NULL, activity_id BIGINT NOT NULL, event_time BIGINT NOT NULL, operation TEXT NOT NULL, status TEXT NOT NULL, lease_until TEXT, created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, completed_at TEXT)").await.unwrap();
    db
}
