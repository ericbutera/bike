use super::QueuedJobReference;
use std::time::Duration;
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions, WorkflowStartOptions};
use temporalio_common::{
    data_converters::{PayloadConverter, RawValue},
    UntypedWorkflow,
};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub async fn connect() -> Result<Client, Error> {
    let address = std::env::var("TEMPORAL_ADDRESS").unwrap_or_else(|_| "http://temporal:7233".into());
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

pub async fn submit(client: &Client, name: &str, input: serde_json::Value) -> Result<QueuedJobReference, Error> {
    let id = format!("bike-{name}-{}", uuid::Uuid::new_v4());
    client
        .start_workflow(
            UntypedWorkflow::new(name),
            RawValue::from_value(&input, &PayloadConverter::default()),
            WorkflowStartOptions::new(crate::platform::temporal::BIKE_TASK_QUEUE, id.clone()).build(),
        )
        .await?;
    Ok(QueuedJobReference {
        id,
        status: "running".into(),
    })
}
