use super::{activities::StravaActivities, StravaActivityDelete, StravaActivityUpsert};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
use temporalio_sdk::{Runtime, Worker, WorkerOptions};

type WorkerError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug)]
pub struct TemporalConfig {
    pub address: String,
    pub namespace: String,
    pub task_queue: String,
}

impl TemporalConfig {
    pub fn from_env() -> Self {
        Self {
            address: std::env::var("TEMPORAL_ADDRESS")
                .unwrap_or_else(|_| "http://temporal:7233".into()),
            namespace: std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into()),
            task_queue: std::env::var("TEMPORAL_TASK_QUEUE")
                .unwrap_or_else(|_| "bike-strava".into()),
        }
    }
}

pub async fn connect(config: &TemporalConfig) -> Result<Client, WorkerError> {
    let options = ConnectionOptions::new(config.address.parse::<temporalio_client::Url>()?)
        .connect_timeout(Duration::from_secs(10))
        .identity("bike-strava-worker")
        .build();
    let connection = Connection::connect(options).await?;
    Ok(Client::new(
        connection,
        ClientOptions::new(config.namespace.clone()).build(),
    )?)
}

pub fn worker(
    runtime: &Runtime,
    client: Client,
    config: &TemporalConfig,
    db: DatabaseConnection,
) -> Result<Worker, WorkerError> {
    let options = WorkerOptions::new(config.task_queue.clone())
        .graceful_shutdown_period(Duration::from_secs(10))
        .register_workflow::<StravaActivityUpsert>()?
        .register_workflow::<StravaActivityDelete>()?
        .register_activities(StravaActivities::new(db))
        .build();
    Ok(Worker::new(runtime, client, options)?)
}
