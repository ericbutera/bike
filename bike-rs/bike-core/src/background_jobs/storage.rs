use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Task status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Processing,
    Canceled,
    Completed,
    Failed,
}

impl TaskStatus {
    pub fn as_str(&self) -> &str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Processing => "processing",
            TaskStatus::Canceled => "canceled",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
        }
    }

    pub fn parse_status(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TaskStatus::Pending),
            "processing" => Some(TaskStatus::Processing),
            "canceled" => Some(TaskStatus::Canceled),
            "completed" => Some(TaskStatus::Completed),
            "failed" => Some(TaskStatus::Failed),
            _ => None,
        }
    }
}

/// Task record stored in the queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub task_type: String,
    pub payload: serde_json::Value,
    pub status: TaskStatus,
    pub attempts: i32,
    pub max_attempts: i32,
    pub error: Option<String>,
    pub scheduled_for: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Trait for task storage implementations
#[async_trait::async_trait]
pub trait TaskStorage: Send + Sync {
    /// Enqueue a new task
    async fn enqueue(
        &self,
        task_type: String,
        payload: serde_json::Value,
        scheduled_for: Option<DateTime<Utc>>,
        max_attempts: i32,
    ) -> Result<TaskRecord, crate::background_jobs::error::TaskError>;

    /// Find pending tasks ready to be processed
    async fn find_pending(
        &self,
        limit: usize,
    ) -> Result<Vec<TaskRecord>, crate::background_jobs::error::TaskError>;

    /// Mark task as processing
    async fn mark_processing(
        &self,
        id: &str,
    ) -> Result<TaskRecord, crate::background_jobs::error::TaskError>;

    /// Mark task as completed
    async fn mark_completed(
        &self,
        id: &str,
    ) -> Result<TaskRecord, crate::background_jobs::error::TaskError>;

    /// Mark task as failed
    async fn mark_failed(
        &self,
        id: &str,
        error: String,
    ) -> Result<TaskRecord, crate::background_jobs::error::TaskError>;

    /// Get task by ID
    async fn get_task(
        &self,
        id: &str,
    ) -> Result<Option<TaskRecord>, crate::background_jobs::error::TaskError>;
}
