//! Database-backed task storage. Enqueue and causal history commit together.
use super::error::TaskError;
use super::pipeline::PipelineContext;
use super::storage::{TaskRecord, TaskStatus as RecordStatus, TaskStorage};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, NotSet, Set,
    TransactionSession, TransactionTrait,
};

pub use super::entities::background_tasks::*;

#[derive(Clone)]
pub struct DurableStorage {
    db: DatabaseConnection,
}

impl DurableStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn task(&self, id: &str) -> Result<Model, TaskError> {
        let id = id
            .parse::<i32>()
            .map_err(|_| TaskError::Storage("Invalid task ID".into()))?;
        Entity::find_by_id(id)
            .one(&self.db)
            .await
            .map_err(storage_error)?
            .ok_or(TaskError::NotFound)
    }
}

impl Model {
    pub async fn enqueue(
        db: &(impl ConnectionTrait + TransactionTrait),
        task_type: String,
        mut payload: serde_json::Value,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) -> Result<Self, DbErr> {
        PipelineContext::attach(&mut payload).map_err(|error| DbErr::Custom(error.to_string()))?;
        if let Some(batch_id) = super::batches::current() {
            payload["_batch_id"] = batch_id.into();
        }
        let context = PipelineContext::from_payload(&payload)
            .map_err(|error| DbErr::Custom(error.to_string()))?;
        let transaction = db.begin().await?;
        let now = Utc::now();
        let task = ActiveModel {
            id: NotSet,
            task_type: Set(task_type),
            payload: Set(payload),
            status: Set(TaskStatus::Pending.as_str().into()),
            attempts: Set(0),
            max_attempts: Set(max_attempts),
            error: Set(None),
            result: Set(None),
            scheduled_for: Set(scheduled_for),
            created_at: Set(now),
            updated_at: Set(now),
            started_at: Set(None),
            completed_at: Set(None),
        }
        .insert(&transaction)
        .await?;
        if let Some(context) = context {
            super::entities::pipeline_runs::Model::record_task(&transaction, &context, task.id)
                .await?;
            super::entities::pipeline_subjects::Model::from_task(
                &transaction,
                &context.run_id,
                &task.payload,
            )
            .await?;
        }
        if let Some(batch_id) = super::batches::current() {
            super::batches::record_task(&transaction, batch_id, task.id).await?;
        }
        transaction.commit().await?;
        Ok(task)
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

    async fn find_pending(&self, limit: usize) -> Result<Vec<TaskRecord>, TaskError> {
        Ok(Model::find_pending(&self.db, limit as u64)
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn mark_processing(&self, id: &str) -> Result<TaskRecord, TaskError> {
        self.task(id)
            .await?
            .mark_processing(&self.db)
            .await
            .map(Into::into)
            .map_err(storage_error)
    }

    async fn mark_completed(&self, id: &str) -> Result<TaskRecord, TaskError> {
        self.task(id)
            .await?
            .mark_completed(&self.db)
            .await
            .map(Into::into)
            .map_err(storage_error)
    }

    async fn mark_failed(&self, id: &str, error: String) -> Result<TaskRecord, TaskError> {
        self.task(id)
            .await?
            .mark_failed(&self.db, error)
            .await
            .map(Into::into)
            .map_err(storage_error)
    }

    async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>, TaskError> {
        match self.task(id).await {
            Ok(task) => Ok(Some(task.into())),
            Err(TaskError::NotFound) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
