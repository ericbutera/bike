//! Direct API-to-Temporal submission. Domain recording belongs to Activities.
use crate::client::{self, Error};
use bike_core::bike_tasks::{ImportSource, JobReference, ReprocessInput, TaskCause};
use temporalio_client::Client;

#[derive(Clone)]
pub struct TaskSubmissionService {
    client: Client,
}

impl TaskSubmissionService {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn process_activity_import(
        &self,
        source: ImportSource,
        cause: TaskCause,
    ) -> Result<JobReference, Error> {
        client::process_activity_import(&self.client, source.with_cause(cause)?).await
    }

    pub async fn reprocess_activity_import(
        &self,
        mut input: ReprocessInput,
        cause: TaskCause,
    ) -> Result<JobReference, Error> {
        input.source = input.source.with_cause(cause)?;
        client::reprocess_activity_import(&self.client, input).await
    }
}
