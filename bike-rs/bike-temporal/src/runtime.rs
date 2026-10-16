//! Bike registration is independent of provider workflows and queues.
use crate::{
    activities::{DerivedActivities, ImportActivities},
    operations::BikeOperations,
    scheduling::SchedulingActivities,
    workflows::{
        DebouncePolicy, PrepareHeatmap, RebuildFitnessFreshness, RebuildSegmentAnalytics,
        ReprocessActivityImport,
    },
    TASK_QUEUE,
};
use bike_core::object_store::{ObjectOwner, ObjectStore};
use std::{sync::Arc, time::Duration};
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
use temporalio_sdk::{Runtime, Worker, WorkerOptions};

pub async fn connect() -> Result<Client, crate::client::Error> {
    let address =
        std::env::var("TEMPORAL_ADDRESS").unwrap_or_else(|_| "http://temporal:7233".into());
    let namespace = std::env::var("TEMPORAL_NAMESPACE").unwrap_or_else(|_| "bike".into());
    let options = ConnectionOptions::new(address.parse::<temporalio_client::Url>()?)
        .identity("bike-tasks")
        .connect_timeout(Duration::from_secs(5))
        .build();
    Ok(Client::new(
        Connection::connect(options).await?,
        ClientOptions::new(namespace).build(),
    )?)
}

pub fn worker(
    runtime: &Runtime,
    client: Client,
    db: sea_orm::DatabaseConnection,
    uploads_dir: String,
) -> Result<Worker, crate::client::Error> {
    let objects = ObjectStore::filesystem(uploads_dir, ObjectOwner::ActivityImports);
    let operations = Arc::new(BikeOperations::new(db, objects));
    let options = WorkerOptions::new(TASK_QUEUE)
        .register_workflow::<ReprocessActivityImport>()?
        .register_workflow::<RebuildFitnessFreshness>()?
        .register_workflow::<RebuildSegmentAnalytics>()?
        .register_workflow::<PrepareHeatmap>()?
        .register_activities(ImportActivities::new(operations.clone()))
        .register_activities(DerivedActivities::new(operations))
        .register_activities(SchedulingActivities::new(
            client.clone(),
            DebouncePolicy::default(),
        ))
        .graceful_shutdown_period(Duration::from_secs(10))
        .build();
    Ok(Worker::new(runtime, client, options)?)
}
