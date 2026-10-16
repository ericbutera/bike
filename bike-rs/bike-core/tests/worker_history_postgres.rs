use bike_core::background_jobs::{
    background_tasks,
    entities::{pipeline_runs, pipeline_tasks, task_attempts},
    pipeline::PipelineContext,
};
use chrono::Duration;
use migration::{Migrator, MigratorTrait, SchemaManager};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait, PaginatorTrait,
    Schema,
};
use serde_json::json;
use uuid::Uuid;

async fn migrated_database() -> (DatabaseConnection, DatabaseConnection, String) {
    let (admin, db, schema) = isolated_database().await;
    let migrations = Migrator::migrations();
    for migration in migrations.iter().filter(|migration| {
        migration.name().contains("pipeline_history")
            || migration.name().contains("pipeline_diagnostics")
    }) {
        migration.up(&SchemaManager::new(&db)).await.unwrap();
    }
    (admin, db, schema)
}
async fn completed_sample(
    db: &DatabaseConnection,
    started: chrono::DateTime<chrono::Utc>,
    seconds: i64,
    version: &str,
    cohort: &str,
) -> background_tasks::Model {
    use sea_orm::{ActiveModelTrait, Set};
    let mut origin = PipelineContext::received("fixture", None);
    origin.pipeline_started_at = started - Duration::minutes(5);
    let pending = PipelineContext::scope(
        Some(origin),
        background_tasks::Model::enqueue(
            db,
            "rebuild_fitness_freshness".into(),
            json!({"data":{"user_id":7}}),
            None,
            3,
        ),
    )
    .await
    .unwrap();
    let completed = pending
        .claim(db)
        .await
        .unwrap()
        .unwrap()
        .mark_completed(db)
        .await
        .unwrap();
    let attempt = task_attempts::Model::for_task(db, completed.id)
        .await
        .unwrap()
        .pop()
        .unwrap();
    let mut attempt: task_attempts::ActiveModel = attempt.into();
    attempt.started_at = Set(started);
    attempt.finished_at = Set(Some(started + Duration::seconds(seconds)));
    attempt.eligible_at = Set(Some(started - Duration::seconds(2)));
    attempt.processing_version = Set(Some(version.into()));
    attempt.workload_cohort = Set(Some(cohort.into()));
    attempt.update(db).await.unwrap();
    completed
}
#[tokio::test]
#[ignore = "Set BIKE_WORKER_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn percentiles_cover_all_attempts_and_empty_registered_processors_stay_unknown() {
    use bike_core::background_jobs::{diagnostics::ProcessorSummary, entities::processor_registry};
    let (admin, db, schema) = migrated_database().await;
    processor_registry::Model::register(
        &db,
        &[
            "rebuild_fitness_freshness".into(),
            "email_notification".into(),
        ],
    )
    .await
    .unwrap();
    let start = chrono::Utc::now() - Duration::hours(2);
    for seconds in 1..=60 {
        completed_sample(
            &db,
            start + Duration::seconds(seconds * 60),
            seconds,
            "1",
            "fixture",
        )
        .await;
    }
    let summary = ProcessorSummary::list(&db, 24, "completed").await.unwrap();
    let fitness = summary
        .iter()
        .find(|p| p.task_type == "rebuild_fitness_freshness")
        .unwrap();
    assert_eq!(fitness.attempt.samples, 60); // More than either UI page.
    assert_eq!(fitness.attempt.p50_seconds, Some(30.5));
    assert!((fitness.attempt.p90_seconds.unwrap() - 54.1).abs() < 0.0001);
    assert_eq!(fitness.eligible_wait.p90_seconds, Some(2.0));
    let empty = summary
        .iter()
        .find(|p| p.task_type == "email_notification")
        .unwrap();
    assert_eq!(empty.attempt.samples, 0);
    assert!(empty.attempt.p90_seconds.is_none());
    let failed = ProcessorSummary::list(&db, 24, "failed").await.unwrap();
    assert!(failed.iter().all(|p| p.attempt.samples == 0));
    let footprint=db.query_one_raw(sea_orm::Statement::from_string(db.get_database_backend(),
  "SELECT sum(pg_total_relation_size(quote_ident(schemaname)||'.'||quote_ident(tablename)))::bigint bytes FROM pg_tables WHERE schemaname=current_schema() AND (tablename LIKE 'processing_%' OR tablename IN ('background_task_attempts','worker_task_anomalies'))".to_owned())).await.unwrap().unwrap().try_get::<i64>("","bytes").unwrap();
    println!(
        "Synthetic diagnostic fixture: 60 runs; total table and index bytes={footprint}; bytes/run={}",
        footprint / 60
    );
    assert!(
        footprint / 60 < 65536,
        "Diagnostic metadata unexpectedly copied large source payloads"
    );
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
async fn isolated_database() -> (DatabaseConnection, DatabaseConnection, String) {
    let url = std::env::var("BIKE_WORKER_TEST_DATABASE_URL").expect("disposable PostgreSQL URL");
    let admin = Database::connect(&url).await.unwrap();
    let schema = format!("worker_history_{}", Uuid::new_v4().simple());
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut options = ConnectOptions::new(url);
    options.set_schema_search_path(&schema);
    let db = Database::connect(options).await.unwrap();
    let definitions = Schema::new(db.get_database_backend());
    db.execute(&definitions.create_table_from_entity(background_tasks::Entity))
        .await
        .unwrap();
    db.execute_unprepared("CREATE TABLE activities (id integer PRIMARY KEY)")
        .await
        .unwrap();
    db.execute(
        &definitions.create_table_from_entity(bike_core::entities::heatmap_projections::Entity),
    )
    .await
    .unwrap();
    db.execute_unprepared("ALTER TABLE heatmap_projections DROP COLUMN published_at")
        .await
        .unwrap();
    db.execute(
        &definitions
            .create_table_from_entity(bike_core::entities::activity_import_attempts::Entity),
    )
    .await
    .unwrap();
    db.execute_unprepared("ALTER TABLE activity_import_attempts DROP COLUMN worker_task_id")
        .await
        .unwrap();
    (admin, db, schema)
}

#[tokio::test]
#[ignore = "Set BIKE_WORKER_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn migration_and_concurrent_claims_preserve_receipt_retry_history_and_foreign_keys() {
    let (admin, db, schema) = isolated_database().await;
    let manager = SchemaManager::new(&db);
    let migrations = Migrator::migrations();
    let migration = migrations
        .iter()
        .find(|migration| migration.name() == "m20261010_000001_pipeline_history")
        .unwrap();
    assert_eq!(migration.name(), "m20261010_000001_pipeline_history");
    migration.up(&manager).await.unwrap();
    let diagnostics = migrations
        .iter()
        .find(|migration| migration.name() == "m20261010_000002_pipeline_diagnostics")
        .unwrap();
    assert_eq!(diagnostics.name(), "m20261010_000002_pipeline_diagnostics");
    diagnostics.up(&manager).await.unwrap();
    let mut origin = PipelineContext::received("upload", Some("postgres-request".into()));
    origin.pipeline_started_at -= Duration::minutes(2);
    let pending = PipelineContext::scope(
        Some(origin.clone()),
        background_tasks::Model::enqueue(&db, "fixture".into(), json!({}), None, 2),
    )
    .await
    .unwrap();
    let (left, right) = tokio::join!(pending.claim(&db), pending.claim(&db));
    let claims: Vec<_> = [left.unwrap(), right.unwrap()]
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(claims.len(), 1);
    let active = claims.into_iter().next().unwrap();
    let retry = active
        .mark_failed(&db, "fixture retry".into())
        .await
        .unwrap();
    let second = retry.claim(&db).await.unwrap().unwrap();
    assert_eq!(
        active.mark_completed(&db).await.unwrap().status,
        "processing"
    );
    second.mark_completed(&db).await.unwrap();
    let attempts = task_attempts::Model::for_task(&db, pending.id)
        .await
        .unwrap();
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].outcome, "retrying");
    assert_eq!(attempts[1].outcome, "completed");
    let root = pipeline_runs::Entity::find_by_id(origin.run_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    // PostgreSQL stores microseconds; the propagated JSON clock keeps nanoseconds.
    assert_eq!(
        root.pipeline_started_at.timestamp_micros(),
        origin.pipeline_started_at.timestamp_micros()
    );
    assert!(root.available_at.is_none());
    assert!(manager
        .has_index("processing_pipeline_runs", "pipeline_runs_trace")
        .await
        .unwrap());
    background_tasks::Entity::delete_by_id(pending.id)
        .exec(&db)
        .await
        .unwrap();
    assert_eq!(pipeline_tasks::Entity::find().count(&db).await.unwrap(), 0);
    assert_eq!(task_attempts::Entity::find().count(&db).await.unwrap(), 0);
    diagnostics.down(&manager).await.unwrap();
    migration.down(&manager).await.unwrap();
    assert!(!manager.has_table("processing_pipeline_runs").await.unwrap());
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
