//! Native Temporal registration and event handoff.
use crate::{activities::StravaActivities, workflows::*};
use bike_core::strava::events::{StravaEvent, StravaOperation};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, WorkflowIdReusePolicy,
    WorkflowStartOptions,
};
use temporalio_common::search_attributes::{SearchAttributeKey, SearchAttributes};
use temporalio_sdk::{Runtime, Worker, WorkerOptions};
use tracing::Instrument;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone, Debug)]
pub struct TemporalConfig {
    pub address: String,
    pub namespace: String,
    pub task_queue: String,
}
impl TemporalConfig {
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            address: lookup("TEMPORAL_ADDRESS").unwrap_or_else(|| "http://temporal:7233".into()),
            namespace: lookup("TEMPORAL_NAMESPACE").unwrap_or_else(|| "bike".into()),
            task_queue: lookup("TEMPORAL_TASK_QUEUE").unwrap_or_else(|| "bike-strava".into()),
        }
    }
}

pub async fn connect(config: &TemporalConfig) -> Result<Client, Error> {
    connect_with_options(connection_options(config)?, &config.namespace).await
}

pub(crate) fn connection_options(config: &TemporalConfig) -> Result<ConnectionOptions, Error> {
    Ok(
        ConnectionOptions::new(config.address.parse::<temporalio_client::Url>()?)
            .identity("bike-strava-temporal")
            .connect_timeout(Duration::from_secs(10))
            .build(),
    )
}

pub(crate) async fn connect_with_options(
    options: ConnectionOptions,
    namespace: &str,
) -> Result<Client, Error> {
    Ok(Client::new(
        Connection::connect(options).await?,
        ClientOptions::new(namespace.to_owned()).build(),
    )?)
}

pub async fn ensure_schedule(client: &Client, config: &TemporalConfig) -> Result<(), Error> {
    use temporalio_client::schedules::{
        CreateScheduleOptions, ScheduleAction, ScheduleError, ScheduleIntervalSpec,
        ScheduleOverlapPolicy, ScheduleSpec,
    };
    let options = CreateScheduleOptions::builder()
        .action(ScheduleAction::start_workflow(
            ScheduledSync::run,
            None,
            config.task_queue.clone(),
            "bike-strava-scan",
        ))
        .spec(
            ScheduleSpec::builder()
                .intervals(vec![ScheduleIntervalSpec::from(Duration::from_secs(
                    6 * 3600,
                ))])
                .build(),
        )
        .overlap_policy(ScheduleOverlapPolicy::Skip)
        .build();
    match client.create_schedule("bike-strava-sync", options).await {
        Ok(_) => Ok(()),
        Err(ScheduleError::Rpc(status)) if status.code() == tonic::Code::AlreadyExists => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub fn worker(
    runtime: &Runtime,
    client: Client,
    config: &TemporalConfig,
    db: DatabaseConnection,
) -> Result<Worker, Error> {
    let options = WorkerOptions::new(config.task_queue.clone())
        .register_workflow::<CreateActivity>()?
        .register_workflow::<UpdateActivity>()?
        .register_workflow::<UpsertActivity>()?
        .register_workflow::<DeleteActivity>()?
        .register_workflow::<SyncActivities>()?
        .register_workflow::<BackfillActivities>()?
        .register_workflow::<DisconnectStrava>()?
        .register_workflow::<ScheduledSync>()?
        .register_activities(StravaActivities::new(db))
        .graceful_shutdown_period(Duration::from_secs(10))
        .build();
    Ok(Worker::new(runtime, client, options)?)
}

pub fn search_attributes(event: &StravaEvent) -> SearchAttributes {
    let mut values = vec![
        SearchAttributeKey::<String>::keyword("BikeCustomerId")
            .value_set(event.bike_customer_id.to_string()),
        SearchAttributeKey::<String>::keyword("BikeUserId")
            .value_set(event.bike_user_id.to_string()),
        SearchAttributeKey::<String>::keyword("StravaAthleteId")
            .value_set(event.strava_athlete_id.to_string()),
        SearchAttributeKey::<String>::keyword("StravaEventId")
            .value_set(event.strava_event_id.to_string()),
        SearchAttributeKey::<String>::keyword("CorrelationId")
            .value_set(event.correlation_id.to_string()),
    ];
    if let Some(id) = event.strava_activity_id {
        values.push(
            SearchAttributeKey::<String>::keyword("StravaActivityId").value_set(id.to_string()),
        );
    }
    SearchAttributes::new(values)
}

pub fn start_options(config: &TemporalConfig, event: &StravaEvent) -> WorkflowStartOptions {
    WorkflowStartOptions::new(config.task_queue.clone(), event.workflow_id())
        .search_attributes(search_attributes(event))
        .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
        .build()
}

pub async fn start_event(
    client: &Client,
    config: &TemporalConfig,
    event: StravaEvent,
) -> Result<Option<String>, Error> {
    bike_core::strava::payload::validate_temporal(&event)?;
    let options = start_options(config, &event);
    let result = match event.operation {
        StravaOperation::Create => client
            .start_workflow(CreateActivity::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
        StravaOperation::Update => client
            .start_workflow(UpdateActivity::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
        StravaOperation::Upsert => client
            .start_workflow(UpsertActivity::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
        StravaOperation::Delete => client
            .start_workflow(DeleteActivity::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
        StravaOperation::Sync => client
            .start_workflow(SyncActivities::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
        StravaOperation::Backfill => client
            .start_workflow(BackfillActivities::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
        StravaOperation::Disconnect => client
            .start_workflow(DisconnectStrava::run, event, options)
            .await
            .map(|handle| handle.run_id().map(str::to_owned)),
    };
    match result {
        Ok(run_id)
        | Err(temporalio_client::errors::WorkflowStartError::AlreadyStarted { run_id, .. }) => {
            Ok(run_id)
        }
        Err(error) => Err(error.into()),
    }
}

pub async fn dispatch_events(
    client: Client,
    config: TemporalConfig,
    db: DatabaseConnection,
) -> Result<(), Error> {
    let metrics = crate::handoff_metrics::HandoffMetrics::new(prometheus::default_registry())?;
    let mut next_sample = tokio::time::Instant::now();
    loop {
        if tokio::time::Instant::now() >= next_sample {
            if let Err(error) = metrics.sample(&db, chrono::Utc::now()).await {
                tracing::error!(error = %error, "Strava handoff backlog sample failed");
            }
            next_sample = tokio::time::Instant::now() + Duration::from_secs(30);
        }
        dispatch_pending(&client, &config, &db).await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn dispatch_pending(
    client: &Client,
    config: &TemporalConfig,
    db: &DatabaseConnection,
) -> Result<(), Error> {
    for row in bike_core::entities::strava_events::Model::pending(db).await? {
        dispatch_event(client, config, db, row).await?;
    }
    Ok(())
}

async fn dispatch_event(
    client: &Client,
    config: &TemporalConfig,
    db: &DatabaseConnection,
    row: bike_core::entities::strava_events::Model,
) -> Result<(), Error> {
    let span = handoff_span(&row, config);
    async {
        let result = match serde_json::from_value(row.event.clone()) {
            Ok(event) => start_event(client, config, event).await,
            Err(error) => Err(error.into()),
        };
        match result {
            Ok(run_id) => {
                tracing::info!(temporal_run_id = run_id, "Strava event handed to Temporal");
                row.submitted(db).await?;
            }
            Err(error) => {
                tracing::error!(error = %error, "Temporal handoff will retry");
                row.retry_handoff(db, error.to_string()).await?;
            }
        }
        Ok(())
    }
    .instrument(span)
    .await
}

pub(crate) fn handoff_span(
    row: &bike_core::entities::strava_events::Model,
    config: &TemporalConfig,
) -> tracing::Span {
    tracing::info_span!("strava.handoff", strava_event_id = row.id,
        correlation_id = row.correlation_id, bike_customer_id = %row.customer_id,
        bike_user_id = row.user_id, strava_athlete_id = row.athlete_id,
        strava_activity_id = row.activity_id, temporal_namespace = config.namespace,
        temporal_workflow_id = row.workflow_id)
}

#[cfg(test)]
mod tests;
