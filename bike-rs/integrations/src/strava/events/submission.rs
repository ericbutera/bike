//! Submit accepted provider events directly to Temporal before acknowledging requests.
use super::{StravaEvent, StravaOperation};
use bike_core::{bike_tasks::JobReference, entities::strava_events, workflow_error::WorkflowError};
use sea_orm::DatabaseConnection;
use std::time::Duration;
use temporalio_client::{
    Client, ClientOptions, Connection, ConnectionOptions, WorkflowIdReusePolicy, WorkflowStartOptions,
};
use temporalio_common::{
    data_converters::{PayloadConverter, RawValue},
    search_attributes::{SearchAttributeKey, SearchAttributes},
    UntypedWorkflow,
};
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

    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
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

pub fn connection_options(config: &TemporalConfig) -> Result<ConnectionOptions, Error> {
    Ok(
        ConnectionOptions::new(config.address.parse::<temporalio_client::Url>()?)
            .identity("bike-strava-temporal")
            .connect_timeout(Duration::from_secs(10))
            .build(),
    )
}

pub async fn connect_with_options(options: ConnectionOptions, namespace: &str) -> Result<Client, Error> {
    Ok(Client::new(
        Connection::connect(options).await?,
        ClientOptions::new(namespace.to_owned()).build(),
    )?)
}

pub fn search_attributes(event: &StravaEvent) -> SearchAttributes {
    let mut values = vec![
        SearchAttributeKey::<String>::keyword("BikeCustomerId").value_set(event.bike_customer_id.to_string()),
        SearchAttributeKey::<String>::keyword("BikeUserId").value_set(event.bike_user_id.to_string()),
        SearchAttributeKey::<String>::keyword("StravaAthleteId").value_set(event.strava_athlete_id.to_string()),
        SearchAttributeKey::<String>::keyword("StravaEventId").value_set(event.strava_event_id.to_string()),
        SearchAttributeKey::<String>::keyword("CorrelationId").value_set(event.correlation_id.to_string()),
    ];
    if let Some(id) = event.strava_activity_id {
        values.push(SearchAttributeKey::<String>::keyword("StravaActivityId").value_set(id.to_string()));
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
    crate::strava::payload::validate_temporal(&event)?;
    let options = start_options(config, &event);
    let workflow = workflow_type(event.operation);
    let result = client
        .start_workflow(
            UntypedWorkflow::new(workflow),
            RawValue::from_value(&event, &PayloadConverter::default()),
            options,
        )
        .await
        .map(|handle| handle.run_id().map(str::to_owned));
    match result {
        Ok(run_id) | Err(temporalio_client::errors::WorkflowStartError::AlreadyStarted { run_id, .. }) => Ok(run_id),
        Err(error) => Err(error.into()),
    }
}

fn workflow_type(operation: StravaOperation) -> &'static str {
    match operation {
        StravaOperation::Create => "CreateActivity",
        StravaOperation::Update => "UpdateActivity",
        StravaOperation::Upsert => "UpsertActivity",
        StravaOperation::Delete => "DeleteActivity",
        StravaOperation::Sync => "SyncActivities",
        StravaOperation::Backfill => "BackfillActivities",
        StravaOperation::Disconnect => "DisconnectStrava",
    }
}

pub async fn accept_with_client(
    client: &Client,
    config: &TemporalConfig,
    db: &DatabaseConnection,
    event: StravaEvent,
) -> Result<strava_events::Model, WorkflowError> {
    let row = strava_events::Model::accept(db, &event).await?;
    submit_accepted(client, config, db, row.clone()).await?;
    Ok(row)
}

pub async fn submit_accepted(
    client: &Client,
    config: &TemporalConfig,
    db: &DatabaseConnection,
    row: strava_events::Model,
) -> Result<JobReference, WorkflowError> {
    let span = handoff_span(&row, config);
    async {
        // A duplicate webhook must use the original accepted event and workflow ID.
        let event: StravaEvent =
            serde_json::from_value(row.event.clone()).map_err(|error| WorkflowError::internal(error.to_string()))?;
        let workflow_type = workflow_type(event.operation).to_string();
        let run_id = start_event(client, config, event)
            .await
            .map_err(submission_error)?
            .ok_or_else(|| WorkflowError::internal("Temporal did not acknowledge a run ID"))?;
        tracing::info!(temporal_run_id = run_id, "Strava event handed to Temporal");
        let job = JobReference {
            job_id: row.workflow_id.clone(),
            namespace: config.namespace.clone(),
            run_id,
            workflow_type,
        };
        row.submitted(db).await?;
        Ok(job)
    }
    .instrument(span)
    .await
}

fn submission_error(error: Error) -> WorkflowError {
    WorkflowError::internal(format!("Strava workflow submission failed: {error}"))
}

pub fn handoff_span(row: &strava_events::Model, config: &TemporalConfig) -> tracing::Span {
    tracing::info_span!("strava.handoff", strava_event_id = row.id,
        correlation_id = row.correlation_id, bike_customer_id = %row.customer_id,
        bike_user_id = row.user_id, strava_athlete_id = row.athlete_id,
        strava_activity_id = row.activity_id, temporal_namespace = config.namespace,
        temporal_workflow_id = row.workflow_id)
}
