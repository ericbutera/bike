//! Causal context and native execution references carried into domain records.
use crate::workflow_error::WorkflowError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

impl TaskCause {
    pub fn validate(&self, user_id: i32) -> Result<(), WorkflowError> {
        if self.user_id != user_id {
            return Err(WorkflowError::bad_request(
                "Affected user does not match command owner",
            ));
        }
        if self.correlation_id.trim().is_empty() {
            return Err(WorkflowError::bad_request("Correlation ID is required"));
        }
        let valid_actor = match &self.actor {
            TaskActor::User(id) => *id > 0,
            TaskActor::System(name) => !name.trim().is_empty(),
        };
        if !valid_actor {
            return Err(WorkflowError::bad_request("Initiating actor is required"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskActor {
    User(i32),
    System(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCause {
    pub correlation_id: String,
    pub request_id: Option<String>,
    pub actor: TaskActor,
    pub user_id: i32,
    pub received_at: DateTime<Utc>,
    pub trace_context: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct JobReference {
    /// Stable public job identity: the acknowledged Temporal Workflow ID.
    pub job_id: String,
    pub namespace: String,
    /// The acknowledged run; Continue-As-New may create subsequent runs.
    pub run_id: String,
    pub workflow_type: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportExecution {
    pub command: super::ActivityImport,
    pub workflow: JobReference,
}

impl ImportExecution {
    /// Run IDs and delivery context can change; the business command cannot.
    pub fn matches_request(&self, other: &Self) -> bool {
        let same_workflow = (
            &self.workflow.job_id,
            &self.workflow.namespace,
            &self.workflow.workflow_type,
        ) == (
            &other.workflow.job_id,
            &other.workflow.namespace,
            &other.workflow.workflow_type,
        );
        let mut command = self.command.clone();
        let mut other_command = other.command.clone();
        command.source.cause = None;
        other_command.source.cause = None;
        same_workflow && command == other_command
    }
}
