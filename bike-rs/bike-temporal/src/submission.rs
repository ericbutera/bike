//! Direct API-to-Temporal submission. Domain recording belongs to Activities.
use crate::client::{self, Error};
use bike_core::bike_tasks::{ActivityImport, ImportSource, ImportTarget, JobReference, ReprocessInput, TaskCause};
use std::sync::Arc;
use temporalio_client::Client;
use tokio::sync::OnceCell;

#[async_trait::async_trait]
pub trait ReprocessSubmission: Send + Sync {
    async fn reprocess_activity_import(&self, input: ReprocessInput, cause: TaskCause) -> Result<JobReference, Error>;
}

#[derive(Clone)]
pub struct TaskSubmissionService {
    client: Arc<OnceCell<Client>>,
}

impl TaskSubmissionService {
    pub fn new(client: Client) -> Self {
        Self {
            client: Arc::new(OnceCell::new_with(Some(client))),
        }
    }

    /// Connecting on first submission keeps API startup independent of Temporal.
    pub fn from_env() -> Self {
        Self {
            client: Arc::new(OnceCell::new()),
        }
    }

    async fn client(&self) -> Result<&Client, Error> {
        self.client.get_or_try_init(crate::runtime::connect).await
    }

    pub async fn process_activity_import(&self, source: ImportSource, cause: TaskCause) -> Result<JobReference, Error> {
        let source = source.with_cause(cause)?;
        ActivityImport {
            source: source.clone(),
            target: ImportTarget::NewActivity,
        }
        .validate()?;
        client::process_activity_import(self.client().await?, source).await
    }

    pub async fn reprocess_activity_import(
        &self,
        mut input: ReprocessInput,
        cause: TaskCause,
    ) -> Result<JobReference, Error> {
        input.source = input.source.with_cause(cause)?;
        input.clone().into_command().validate()?;
        client::reprocess_activity_import(self.client().await?, input).await
    }
}

#[async_trait::async_trait]
impl ReprocessSubmission for TaskSubmissionService {
    async fn reprocess_activity_import(&self, input: ReprocessInput, cause: TaskCause) -> Result<JobReference, Error> {
        TaskSubmissionService::reprocess_activity_import(self, input, cause).await
    }
}
