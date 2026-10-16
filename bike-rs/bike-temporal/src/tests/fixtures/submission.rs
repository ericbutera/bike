use super::super::client::{client, Requests};
use crate::{client::Error, submission::TaskSubmissionService};
use bike_core::{
    bike_tasks::{JobReference, ReprocessInput},
    test_support as fixtures,
};
use std::sync::{Arc, Mutex};

pub(crate) struct SubmissionFixture {
    pub service: TaskSubmissionService,
    pub requests: Arc<Mutex<Requests>>,
}

impl SubmissionFixture {
    pub async fn new() -> Self {
        let requests = Arc::new(Mutex::new(Requests::default()));
        let service = TaskSubmissionService::new(client(requests.clone(), false).await);
        Self { service, requests }
    }

    pub async fn process_activity_import(&self) -> Result<JobReference, Error> {
        self.service
            .process_activity_import(fixtures::import_command().source, fixtures::task_cause())
            .await
    }

    pub async fn reprocess_activity_import(&self) -> Result<JobReference, Error> {
        self.service
            .reprocess_activity_import(reprocess_input(), fixtures::task_cause())
            .await
    }
}

pub(crate) fn reprocess_input() -> ReprocessInput {
    ReprocessInput {
        source: fixtures::import_command().source,
        activity_id: fixtures::import_result().activity_id,
        expected_revision: 3,
    }
}
