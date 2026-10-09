//! A command-line fixture boundary, never an application HTTP endpoint.
//! Playwright owns scenario values; SeaORM validates and persists owning models.
use bike_core::auth::entities::{refresh_tokens, users};
use bike_core::background_jobs::entities::background_tasks;
use bike_core::entities::{
    activities, activity_archive_import_jobs, activity_import_artifacts, activity_imports,
    heatmap_projections, segment_efforts, segment_summaries, segment_user_summaries, segments,
    user_preferences,
};
use bike_core::heatmaps::{preparation::prepare_activity, projection::PendingProjection};
use sea_orm::{
    ActiveModelTrait, DatabaseTransaction, EntityTrait, IntoActiveModel, TransactionTrait,
    TryIntoModel,
};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dataset {
    records: Vec<Record>,
    #[serde(default)]
    prepare_heatmaps: bool,
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
        "background_tasks" => store::<background_tasks::Entity>(tx, record).await,
        "activity_archive_import_jobs" => {
            store::<activity_archive_import_jobs::Entity>(tx, record).await
        }
        "refresh_tokens" => store::<refresh_tokens::Entity>(tx, record).await,
        entity => Err(sea_orm::DbErr::Custom(format!(
            "Unsupported fixture entity: {entity}"
        ))),
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

async fn seed(db: &sea_orm::DatabaseConnection, dataset: Dataset) -> Result<(), sea_orm::DbErr> {
    let tx = db.begin().await?;
    for record in dataset.records {
        store_record(&tx, record).await?;
    }
    tx.commit().await?;
    if dataset.prepare_heatmaps {
        // Reuse the owning projection builder to create persisted result data;
        // browser tests do not run a worker or wait for asynchronous jobs.
        prepare_heatmaps(db).await?;
    }
    Ok(())
}

async fn prepare_heatmaps(db: &sea_orm::DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    for _ in 0..3 {
        let projections = heatmap_projections::Entity::find().all(db).await?;
        if projections
            .iter()
            .all(|projection| projection.status == "ready")
        {
            return Ok(());
        }
        for projection in projections {
            if projection.status != "ready" {
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
        "Fixture heatmap projections did not become ready".into(),
    ))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let project = std::env::var("BIKE_E2E_PROJECT")?;
    if !project.starts_with("bike-e2e-") {
        return Err("Fixture loading requires a disposable E2E project".into());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let dataset = serde_json::from_str(&input)?;
    let db = bike_core::db::connect_database(&std::env::var("DATABASE_URL")?).await?;
    seed(&db, dataset).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database, Schema};
    use serde_json::json;

    #[tokio::test]
    async fn owning_model_inserts_and_updates_partial_fixture_values() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let backend = db.get_database_backend();
        db.execute(&Schema::new(backend).create_table_from_entity(users::Entity))
            .await
            .unwrap();
        let dataset = serde_json::from_value(json!({"records": [{
            "entity": "users", "operation": "insert", "values": {
                "id": 1, "pid": "00000000-0000-4000-8000-000000000001",
                "email": "rider@e2e.test", "api_key": "fixture", "name": "Rider",
                "disabled": false, "created_at": "2025-01-01T00:00:00Z",
                "updated_at": "2025-01-01T00:00:00Z"
            }
        }, {"entity": "users", "operation": "update", "values": {
            "id": 1, "name": "Updated rider"
        }}]}))
        .unwrap();
        seed(&db, dataset).await.unwrap();
        let user = users::Entity::find_by_id(1)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.name, "Updated rider");
        assert_eq!(user.email, "rider@e2e.test");
    }

    #[test]
    fn owning_models_reject_stale_fixture_fields_and_invalid_values() {
        assert!(users::ActiveModel::from_json(json!({"removed_column": 1})).is_err());
        assert!(users::ActiveModel::from_json(json!({"disabled": "yes"})).is_err());
    }
}
