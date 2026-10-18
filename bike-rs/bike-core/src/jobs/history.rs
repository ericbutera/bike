//! A read-only view of retained Temporal executions; no local execution records.
use super::{native::Error, JobQueue};
use crate::bike_tasks::JobReference;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use temporalio_client::NamespacedClient;
use temporalio_common::protos::temporal::api::{
    common::v1::WorkflowExecution,
    history::v1::history_event::Attributes,
    workflow::v1::WorkflowExecutionInfo,
    workflowservice::v1::{GetWorkflowExecutionHistoryRequest, ListWorkflowExecutionsRequest},
};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema)]
pub enum TaskStatus {
    Running,
    Completed,
    Failed,
    Canceled,
    Terminated,
    ContinuedAsNew,
    TimedOut,
    Unspecified,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskExecution {
    pub job: JobReference,
    pub status: TaskStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskExecutionPage {
    pub executions: Vec<TaskExecution>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TaskExecutionInput {
    pub parameters: Vec<serde_json::Value>,
}

impl JobQueue {
    pub async fn execution_history(
        &self,
        status: Option<TaskStatus>,
        page_token: Vec<u8>,
        user_id: Option<i32>,
    ) -> Result<TaskExecutionPage, Error> {
        let client = self.client().await?;
        let namespace = client.namespace();
        let response = client
            .connection()
            .workflow_service()
            .list_workflow_executions(tonic::Request::new(ListWorkflowExecutionsRequest {
                namespace: namespace.clone(),
                page_size: 25,
                next_page_token: page_token,
                query: visibility_query(status, user_id),
            }))
            .await?
            .into_inner();
        Ok(TaskExecutionPage {
            executions: response
                .executions
                .into_iter()
                .map(|info| execution(&namespace, info))
                .collect(),
            next_page_token: (!response.next_page_token.is_empty()).then(|| hex::encode(response.next_page_token)),
        })
    }

    pub async fn execution_input(&self, workflow_id: String, run_id: String) -> Result<TaskExecutionInput, Error> {
        let client = self.client().await?;
        let response = client
            .connection()
            .workflow_service()
            .get_workflow_execution_history(tonic::Request::new(GetWorkflowExecutionHistoryRequest {
                namespace: client.namespace(),
                execution: Some(WorkflowExecution { workflow_id, run_id }),
                maximum_page_size: 1,
                ..Default::default()
            }))
            .await?
            .into_inner();
        let first = response.history.and_then(|history| history.events.into_iter().next());
        let Some(Attributes::WorkflowExecutionStartedEventAttributes(started)) =
            first.and_then(|event| event.attributes)
        else {
            return Err("Workflow input is no longer available".into());
        };
        let queue = started.task_queue.map(|queue| queue.name).unwrap_or_default();
        if !task_queues().contains(&queue) {
            return Err("Workflow does not belong to a Bike task queue".into());
        }
        let parameters = started
            .input
            .unwrap_or_default()
            .payloads
            .into_iter()
            .map(|payload| match payload.metadata.get("encoding").map(Vec::as_slice) {
                Some(b"json/plain") => serde_json::from_slice(&payload.data).map_err(Error::from),
                Some(b"binary/null") => Ok(serde_json::Value::Null),
                _ => Err("Workflow input encoding is not supported".into()),
            })
            .collect::<Result<_, Error>>()?;
        Ok(TaskExecutionInput { parameters })
    }
}

fn task_queues() -> [String; 2] {
    [
        crate::platform::temporal::BIKE_TASK_QUEUE.into(),
        std::env::var("TEMPORAL_TASK_QUEUE").unwrap_or_else(|_| "bike-strava".into()),
    ]
}

fn visibility_query(status: Option<TaskStatus>, user_id: Option<i32>) -> String {
    let queues = task_queues().map(|queue| queue.replace('\'', "''"));
    let mut query = format!("TaskQueue IN ('{}', '{}')", queues[0], queues[1]);
    if let Some(status) = status {
        query.push_str(&format!(" AND ExecutionStatus = '{status:?}'"));
    }
    if let Some(user_id) = user_id {
        query.push_str(&format!(" AND BikeUserId = '{user_id}'"));
    }
    query
}

fn execution(namespace: &str, info: WorkflowExecutionInfo) -> TaskExecution {
    let identity = info.execution.unwrap_or_default();
    TaskExecution {
        job: JobReference {
            job_id: identity.workflow_id,
            run_id: identity.run_id,
            namespace: namespace.into(),
            workflow_type: info.r#type.unwrap_or_default().name,
        },
        status: match info.status {
            1 => TaskStatus::Running,
            2 => TaskStatus::Completed,
            3 => TaskStatus::Failed,
            4 => TaskStatus::Canceled,
            5 => TaskStatus::Terminated,
            6 => TaskStatus::ContinuedAsNew,
            7 => TaskStatus::TimedOut,
            _ => TaskStatus::Unspecified,
        },
        started_at: info
            .start_time
            .and_then(|time| DateTime::from_timestamp(time.seconds, time.nanos as u32)),
        closed_at: info
            .close_time
            .and_then(|time| DateTime::from_timestamp(time.seconds, time.nanos as u32)),
    }
}

#[cfg(test)]
mod tests;
