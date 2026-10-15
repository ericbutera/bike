//! Database-backed task storage. Enqueue and causal history commit together.
use super::error::TaskError;
use super::storage::{TaskRecord, TaskStatus as RecordStatus, TaskStorage};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr};

pub use super::entities::background_tasks::*;

#[derive(Clone)]
pub struct DurableStorage {
    db: DatabaseConnection,
}

impl DurableStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl From<Model> for TaskRecord {
    fn from(model: Model) -> Self {
        Self {
            id: model.id.to_string(),
            task_type: model.task_type,
            payload: model.payload,
            status: RecordStatus::parse_status(&model.status).unwrap_or(RecordStatus::Pending),
            attempts: model.attempts,
            max_attempts: model.max_attempts,
            error: model.error,
            scheduled_for: model.scheduled_for,
            created_at: model.created_at,
            updated_at: model.updated_at,
            started_at: model.started_at,
            completed_at: model.completed_at,
        }
    }
}

fn storage_error(error: DbErr) -> TaskError {
    TaskError::Storage(error.to_string())
}

#[async_trait]
impl TaskStorage for DurableStorage {
    async fn enqueue(
        &self,
        task_type: String,
        payload: serde_json::Value,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) -> Result<TaskRecord, TaskError> {
        Model::enqueue(&self.db, task_type, payload, scheduled_for, max_attempts)
            .await
            .map(Into::into)
            .map_err(storage_error)
    }
}
