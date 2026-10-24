//! Fast connected acceptance using the existing E2E database and Temporal server.
#[path = "activity_dag/provider.rs"]
mod provider;
#[path = "activity_dag/standings.rs"]
mod standings;
#[path = "activity_dag/verify.rs"]
mod verify;
use bike_core::{
    auth::entities::users,
    entities::{strava_connections, strava_events},
};
use integrations::strava::{native::StravaOperations, objects::ObjectStore};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use strava_temporal::client::{connect, start_options, worker_with_operations, TemporalConfig};
use temporalio_client::{Client, WorkflowGetResultOptions};
use temporalio_sdk::{runtime::RuntimeOptions, Runtime};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    reference: Value,
    provider_payload: Value,
}

#[derive(Default, Serialize)]
pub(super) struct Report {
    pub failures: Vec<String>,
    passed: Vec<String>,
    workflow_ms: Vec<u128>,
    comparisons: Value,
}

impl Report {
    fn check(&mut self, name: impl Into<String>, passed: bool) {
        if passed {
            self.passed.push(name.into());
        } else {
            self.failures.push(name.into());
        }
    }
}

pub(super) async fn run(db: &DatabaseConnection, input: Input) -> Result<Report> {
    provider::seed(db, &input).await?;
    let app = bike_core::config::Config::get();
    let config = TemporalConfig::from_env();
    let client = connect(&config).await?;
    let store = Arc::new(ObjectStore::new(&app.uploads_dir));
    let provider = Arc::new(provider::FrozenProvider::new(&store, &input.provider_payload).await?);
    let objects = bike_core::object_store::ObjectStore::filesystem(
        &app.uploads_dir,
        bike_core::object_store::ObjectOwner::Strava,
    );
    let operations = StravaOperations::with_dependencies(db.clone(), provider, store, objects);
    let runtime = Runtime::from_current_tokio(RuntimeOptions::default())?;
    let mut strava = worker_with_operations(&runtime, client.clone(), &config, operations)?;
    let mut bike = bike_temporal::runtime::worker(&runtime, client.clone(), db.clone(), app.uploads_dir.clone())?;
    let stop_strava = strava.shutdown_handle();
    let stop_bike = bike.shutdown_handle();
    let execute = async {
        let result = tokio::time::timeout(Duration::from_secs(30), compare_runs(db, &client, &config, &input)).await;
        stop_strava();
        stop_bike();
        result?
    };
    let (report, strava_result, bike_result) = tokio::join!(execute, strava.run(), bike.run());
    strava_result?;
    bike_result?;
    report
}

async fn compare_runs(
    db: &DatabaseConnection,
    client: &Client,
    config: &TemporalConfig,
    input: &Input,
) -> Result<Report> {
    let mut report = Report::default();
    let mut saved = None;
    for attempt in 1..=2 {
        let event = event(db, input).await?;
        let start = Instant::now();
        let handle = client
            .start_workflow(
                strava_temporal::workflows::UpsertActivity::run,
                event.clone(),
                start_options(config, &event),
            )
            .await?;
        let change = handle.get_result(WorkflowGetResultOptions::default()).await?;
        report.workflow_ms.push(start.elapsed().as_millis());
        report.check(
            format!("run {attempt}: original activity identity"),
            change.bike_activity_id == input.reference["activity"]["id"].as_i64().map(|id| id as i32),
        );
        let current = verify::saved_outputs(db, input, &mut report).await?;
        standings::verify(db, input, &mut report).await?;
        if let Some(previous) = saved {
            report.check(
                "unchanged retry preserves effort IDs, awards and recording reference",
                previous == current,
            );
        }
        saved = Some(current);
    }
    Ok(report)
}

async fn event(db: &DatabaseConnection, input: &Input) -> Result<strava_events::StravaEvent> {
    let user_id = input.reference["activity"]["user_id"]
        .as_i64()
        .ok_or("Reference owner missing")? as i32;
    let user = users::Model::find_by_id(db, user_id).await?;
    let connection = strava_connections::Model::for_user(db, user_id)
        .await?
        .ok_or("Fixture connection missing")?;
    let mut event = strava_events::StravaEvent::new(&user, &connection, strava_events::StravaOperation::Upsert);
    event.strava_activity_id = Some(
        input.provider_payload["activity"]["id"]
            .as_i64()
            .ok_or("Provider activity ID missing")?,
    );
    strava_events::Model::accept(db, &event).await?;
    Ok(event)
}
