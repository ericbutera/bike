//! A command-line fixture boundary, never an application HTTP endpoint.
//! Playwright owns scenario values; SeaORM validates and persists owning models.
use bike_core::auth::entities::{refresh_tokens, users};
use bike_core::entities::{
    activities, activity_archive_import_jobs, activity_import_artifacts, activity_imports, heatmap_projections,
    segment_efforts, segment_summaries, segment_user_summaries, segments, user_preferences,
};
use bike_core::heatmaps::{preparation::prepare_activity, projection::PendingProjection};
use sea_orm::{ActiveModelTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, TransactionTrait, TryIntoModel};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[path = "e2e-fixtures/activity_dag.rs"]
mod activity_dag;
#[path = "e2e-fixtures/segment_matching.rs"]
mod segment_matching;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dataset {
    records: Vec<Record>,
    #[serde(default)]
    prepare_heatmaps: bool,
    #[serde(default)]
    segment_matching: Option<segment_matching::Input>,
    #[serde(default)]
    reconcile_pairs: Vec<ActivitySegmentPair>,
    #[serde(default)]
    prepare_recordings: Vec<bike_core::bike_tasks::ActivityTask>,
    #[serde(default)]
    activity_dag: Option<activity_dag::Input>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActivitySegmentPair {
    user_id: i32,
    activity_id: i32,
    segment_id: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    entity: String,
    operation: Operation,
    values: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Operation {
    Insert,
    Update,
}

async fn store_record(tx: &DatabaseTransaction, record: Record) -> Result<(), sea_orm::DbErr> {
    // Native from_json preserves omitted columns as NotSet, so database defaults
    // remain authoritative. Model serde rejects unknown fields and wrong types.
    match record.entity.as_str() {
        "users" => store::<users::Entity>(tx, record).await,
        "user_preferences" => store::<user_preferences::Entity>(tx, record).await,
        "activity_imports" => store::<activity_imports::Entity>(tx, record).await,
        "activity_import_artifacts" => store::<activity_import_artifacts::Entity>(tx, record).await,
        "activities" => store::<activities::Entity>(tx, record).await,
        "segments" => store::<segments::Entity>(tx, record).await,
        "segment_efforts" => store::<segment_efforts::Entity>(tx, record).await,
        "segment_summaries" => store::<segment_summaries::Entity>(tx, record).await,
        "segment_user_summaries" => store::<segment_user_summaries::Entity>(tx, record).await,
        "activity_archive_import_jobs" => store::<activity_archive_import_jobs::Entity>(tx, record).await,
        "refresh_tokens" => store::<refresh_tokens::Entity>(tx, record).await,
        entity => Err(sea_orm::DbErr::Custom(format!("Unsupported fixture entity: {entity}"))),
    }
}

async fn store<E>(tx: &DatabaseTransaction, record: Record) -> Result<(), sea_orm::DbErr>
where
    E: EntityTrait,
    E::ActiveModel: TryIntoModel<E::Model>,
    E::Model: Serialize + for<'de> Deserialize<'de> + IntoActiveModel<E::ActiveModel>,
{
    let model = E::ActiveModel::from_json(record.values)?;
    match record.operation {
        Operation::Insert => {
            E::insert(model).exec(tx).await?;
        }
        Operation::Update => {
            E::update(model).exec(tx).await?;
        }
    }
    Ok(())
}

async fn seed(
    db: &sea_orm::DatabaseConnection,
    dataset: Dataset,
    initialize_outputs: bool,
) -> Result<(), sea_orm::DbErr> {
    let activity_ids: Vec<i32> = dataset
        .records
        .iter()
        .filter(|record| record.entity == "activities")
        .filter_map(|record| record.values.get("id")?.as_i64()?.try_into().ok())
        .collect();
    let segment_ids: Vec<i32> = dataset
        .records
        .iter()
        .filter(|record| record.entity == "segments")
        .filter_map(|record| record.values.get("id")?.as_i64()?.try_into().ok())
        .collect();
    let tx = db.begin().await?;
    for record in dataset.records {
        store_record(&tx, record).await?;
    }
    tx.commit().await?;
    if !initialize_outputs {
        return Ok(());
    }
    prepare_climbs(db, activity_ids).await?;
    for id in segment_ids {
        bike_core::personal_achievements::rebuild(db, id).await?;
        bike_core::overall_achievements::rebuild(db, id).await?;
    }
    if dataset.prepare_heatmaps {
        // Reuse the owning projection builder to create persisted result data;
        // browser tests do not run a worker or wait for asynchronous jobs.
        prepare_heatmaps(db).await?;
    }
    Ok(())
}

async fn prepare_climbs(db: &sea_orm::DatabaseConnection, activity_ids: Vec<i32>) -> Result<(), sea_orm::DbErr> {
    let objects = bike_core::object_store::ObjectStore::filesystem(
        &bike_core::config::Config::get().uploads_dir,
        bike_core::object_store::ObjectOwner::ActivityImports,
    );
    for id in activity_ids {
        let activity = activities::Entity::find_by_id(id)
            .one(db)
            .await?
            .ok_or_else(|| sea_orm::DbErr::Custom("Fixture activity missing after seed".into()))?;
        bike_core::activity_climbs::rebuild(
            db,
            &objects,
            bike_core::bike_tasks::ActivityTask {
                activity_id: activity.id,
                user_id: activity.user_id,
                priority: bike_core::bike_tasks::Priority::Background,
            },
        )
        .await
        .map_err(|error| sea_orm::DbErr::Custom(error.message))?;
    }
    Ok(())
}

async fn prepare_heatmaps(db: &sea_orm::DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    for _ in 0..3 {
        let projections = heatmap_projections::Entity::find().all(db).await?;
        if projections
            .iter()
            .all(|projection| matches!(projection.status.as_str(), "ready" | "skipped"))
        {
            return Ok(());
        }
        for projection in projections {
            if !matches!(projection.status.as_str(), "ready" | "skipped") {
                prepare_activity(
                    db,
                    PendingProjection {
                        activity_id: projection.activity_id,
                        generation: projection.generation,
                    },
                )
                .await?;
            }
        }
    }
    Err(sea_orm::DbErr::Custom(
        "Fixture heatmap projections did not finish preparation".into(),
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let staging = fixture_mode(
        std::env::var("BIKE_E2E_PROJECT").ok().as_deref(),
        std::env::var("BIKE_FIXTURE_TARGET").ok().as_deref(),
        std::env::var("APP_ENV").ok().as_deref(),
    )?;
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let mut dataset: Dataset = serde_json::from_str(&input)?;
    let matching = dataset.segment_matching.take();
    let activity_dag = dataset.activity_dag.take();
    let pairs = std::mem::take(&mut dataset.reconcile_pairs);
    let recordings = std::mem::take(&mut dataset.prepare_recordings);
    let acceptance = matching.is_some() || !pairs.is_empty() || !recordings.is_empty();
    let acceptance = acceptance || activity_dag.is_some();
    if staging && acceptance {
        return Err("Matching acceptance requires a disposable E2E project".into());
    }
    let db = bike_core::db::connect_database(&std::env::var("DATABASE_URL")?).await?;
    if staging && staging_already_seeded(&db, &dataset).await? {
        if dataset.prepare_heatmaps {
            prepare_heatmaps(&db).await?;
        }
        println!("Existing staging fixtures preserved");
        return Ok(());
    }
    seed(&db, dataset, activity_dag.is_none()).await?;
    let objects = bike_core::object_store::ObjectStore::filesystem(
        &bike_core::config::Config::get().uploads_dir,
        bike_core::object_store::ObjectOwner::ActivityImports,
    );
    prepare_matching(&db, &objects, recordings, pairs).await?;
    if let Some(input) = activity_dag {
        let report = activity_dag::run(&db, input)
            .await
            .map_err(|error| -> Box<dyn std::error::Error> { error })?;
        println!("{}", serde_json::to_string(&report)?);
        if !report.failures.is_empty() {
            return Err("Activity DAG regression failed; see the comparison report".into());
        }
    }
    if let Some(input) = matching {
        println!("{}", serde_json::to_string(&segment_matching::run(&db, input).await?)?);
    }
    Ok(())
}

fn fixture_mode(project: Option<&str>, target: Option<&str>, app_env: Option<&str>) -> Result<bool, &'static str> {
    match target {
        Some("staging") if app_env == Some("staging") => Ok(true),
        Some(_) => Err("Staging fixtures require BIKE_FIXTURE_TARGET=staging and APP_ENV=staging"),
        None if project.is_some_and(|value| value.starts_with("bike-e2e-")) => Ok(false),
        None => Err("Fixture loading requires a disposable E2E project or the staging target"),
    }
}

fn require_staging_database(name: &str) -> Result<(), sea_orm::DbErr> {
    if name != "bike_staging" {
        return Err(sea_orm::DbErr::Custom(
            "Staging fixtures require database bike_staging".into(),
        ));
    }
    Ok(())
}

async fn staging_already_seeded(db: &sea_orm::DatabaseConnection, dataset: &Dataset) -> Result<bool, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, QueryOrder, Statement};
    let row = db
        .query_one_raw(Statement::from_string(
            DatabaseBackend::Postgres,
            "SELECT current_database() AS database_name",
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("Cannot verify fixture database".into()))?;
    require_staging_database(&row.try_get::<String>("", "database_name")?)?;
    let owner = dataset.records.iter().find(|record| {
        record.entity == "users" && record.values.get("id").and_then(serde_json::Value::as_i64) == Some(1)
    });
    let email = owner
        .and_then(|record| record.values.get("email"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| sea_orm::DbErr::Custom("Staging dataset requires its fixture owner".into()))?;
    match users::Entity::find().order_by_asc(users::Column::Id).one(db).await? {
        None => Ok(false),
        Some(user) if user.id == 1 && user.email == email => Ok(true),
        Some(_) => Err(sea_orm::DbErr::Custom(
            "Refusing to seed a nonempty database with a different fixture owner".into(),
        )),
    }
}

async fn prepare_matching(
    db: &sea_orm::DatabaseConnection,
    objects: &bike_core::object_store::ObjectStore,
    recordings: Vec<bike_core::bike_tasks::ActivityTask>,
    pairs: Vec<ActivitySegmentPair>,
) -> Result<(), Box<dyn std::error::Error>> {
    for task in recordings {
        bike_core::entities::activity_recordings::Model::backfill(db, objects, task).await?;
    }
    for pair in pairs {
        reconcile_pair(db, objects, pair).await?;
    }
    Ok(())
}

async fn reconcile_pair(
    db: &sea_orm::DatabaseConnection,
    objects: &bike_core::object_store::ObjectStore,
    pair: ActivitySegmentPair,
) -> Result<(), Box<dyn std::error::Error>> {
    let segment = segments::Model::find_by_id(db, pair.segment_id)
        .await?
        .ok_or("Pair reconciliation segment is missing")?;
    let route = bike_core::segment_support::deserialize_segment_route_points(segment.route_data_json.as_ref());
    bike_core::segment_support::reconcile_activity_segment(
        db,
        objects,
        pair.user_id,
        pair.activity_id,
        pair.segment_id,
        &route,
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bike_core::{entities::activity_recordings, object_store::ObjectOwner, test_support as fixtures};

    #[tokio::test]
    async fn matching_fixture_uses_the_recording_producer_and_shared_pair_writer() {
        let data = fixtures::parsed_reprocess_activity().derived_data;
        let activity = activities::Model {
            derived_data_json: Some((&data).into()),
            ..fixtures::activity()
        };
        let segment = segments::Model {
            route_data_json: Some(bike_core::segment_support::serialize_segment_route_points(
                &data.route_points,
            )),
            ..fixtures::segment()
        };
        let (objects, _) = fixtures::object_storage(ObjectOwner::BikeTasks);
        let reference = objects
            .transform(
                &format!("bike/tasks/recordings/{}", activity.user_id),
                &activity_recordings::Recording::from(data),
            )
            .await
            .unwrap();
        let recording = fixtures::activity_recording(&reference);
        let dataset: Dataset = serde_json::from_value(serde_json::json!({
            "records": [],
            "prepare_recordings": [{"user_id": activity.user_id, "activity_id": activity.id, "priority": "Background"}],
            "reconcile_pairs": [{"user_id": activity.user_id, "activity_id": activity.id, "segment_id": segment.id}]
        }))
        .unwrap();
        let db = fixtures::postgres_mock()
            .append_query_results([vec![activity.clone()]])
            .append_query_results([vec![recording.clone()]])
            .append_query_results([vec![segment.clone()]])
            .append_query_results([vec![activity]])
            .append_query_results([vec![recording]])
            .append_query_results([vec![segment]])
            .append_query_results([Vec::<segment_efforts::Model>::new()])
            .append_exec_results([fixtures::written(), fixtures::written()])
            .into_connection();
        prepare_matching(&db, &objects, dataset.prepare_recordings, dataset.reconcile_pairs)
            .await
            .unwrap();
        let transactions = db.into_transaction_log();
        assert_eq!(transactions.len(), 3);
        let statements: Vec<_> = transactions.iter().flat_map(|txn| txn.statements()).collect();
        assert!(statements
            .iter()
            .any(|stmt| stmt.sql.starts_with("INSERT INTO \"segment_efforts\"")));
        assert!(!statements.iter().any(|stmt| stmt.sql.starts_with("DELETE")));
        assert!(!statements
            .iter()
            .any(|stmt| stmt.sql.starts_with("INSERT INTO \"activity_recordings\"")));
    }
}
