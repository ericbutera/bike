use bike_core::{background_jobs::entities::*, entities::*};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, Schema};

pub async fn database() -> DatabaseConnection {
    // Clock-driven sampler tests must keep the connection that owns this
    // in-memory database when they advance beyond the pool's default lifetime.
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None);
    let db = Database::connect(options).await.unwrap();
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
        schema.create_table_from_entity(strava_activity_states::Entity),
        schema.create_table_from_entity(integration_events::Entity),
        schema.create_table_from_entity(bike_core::platform::feature_flags::entities::Entity),
    ] {
        db.execute(&statement).await.unwrap();
    }
    // These receipt tables use migration-owned database clock defaults. Entity
    // schema generation does not include those defaults.
    db
}
