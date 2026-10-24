//! Native Temporal worker registration. Requests submit workflows directly.
use crate::{activities::StravaActivities, workflows::*};
pub use integrations::strava::events::submission::{
    connect, connect_with_options, connection_options, handoff_span, search_attributes, start_event, start_options,
    Error, TemporalConfig,
};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use temporalio_client::Client;
use temporalio_sdk::{Runtime, Worker, WorkerOptions};

pub fn worker(
    runtime: &Runtime,
    client: Client,
    config: &TemporalConfig,
    db: DatabaseConnection,
) -> Result<Worker, Error> {
    worker_with_operations(
        runtime,
        client,
        config,
        integrations::strava::native::StravaOperations::new(db),
    )
}

pub fn worker_with_operations(
    runtime: &Runtime,
    client: Client,
    config: &TemporalConfig,
    operations: integrations::strava::native::StravaOperations,
) -> Result<Worker, Error> {
    let options = WorkerOptions::new(config.task_queue.clone())
        .register_workflow::<CreateActivity>()?
        .register_workflow::<UpdateActivity>()?
        .register_workflow::<UpsertActivity>()?
        .register_workflow::<ReconcileActivity>()?
        .register_workflow::<DeleteActivity>()?
        .register_workflow::<SyncActivities>()?
        .register_workflow::<BackfillActivities>()?
        .register_workflow::<DisconnectStrava>()?
        .register_workflow::<ScheduledSync>()?
        .register_activities(StravaActivities::with_operations(operations))
        .graceful_shutdown_period(Duration::from_secs(10))
        .build();
    Ok(Worker::new(runtime, client, options)?)
}

#[cfg(test)]
mod tests;
