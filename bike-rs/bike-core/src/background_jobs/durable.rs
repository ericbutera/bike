// PostgreSQL-backed durable task storage
//
// This implementation stores tasks in a database table for persistence
// and durability. Tasks survive application restarts.

use crate::background_jobs::error::TaskError;
use crate::background_jobs::storage::{TaskRecord, TaskStatus, TaskStorage};
use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, NotSet, QueryFilter,
    QueryOrder, QuerySelect, Set,
};

// Re-export the background_tasks entity
pub use background_tasks_entity::*;

mod background_tasks_entity {
    use chrono::{DateTime, Utc};
    use sea_orm::entity::prelude::*;

    #[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
    #[sea_orm(table_name = "background_tasks")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub task_type: String,
        pub payload: Json,
        pub status: String,
        pub attempts: i32,
        pub max_attempts: i32,
        pub error: Option<String>,
        pub scheduled_for: Option<DateTime<Utc>>,
        pub created_at: DateTime<Utc>,
        pub updated_at: DateTime<Utc>,
        pub started_at: Option<DateTime<Utc>>,
        pub completed_at: Option<DateTime<Utc>>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// Durable storage backed by PostgreSQL
#[derive(Clone)]
pub struct DurableStorage {
    db: DatabaseConnection,
}

impl DurableStorage {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
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
        let active_model = ActiveModel {
            id: NotSet,
            task_type: Set(task_type),
            payload: Set(payload.clone()),
            status: Set(TaskStatus::Pending.as_str().to_string()),
            attempts: Set(0),
            max_attempts: Set(max_attempts),
            error: Set(None),
            scheduled_for: Set(scheduled_for),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            started_at: Set(None),
            completed_at: Set(None),
        };

        let model = active_model
            .insert(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?;

        Ok(TaskRecord {
            id: model.id.to_string(),
            task_type: model.task_type,
            payload,
            status: TaskStatus::parse_status(&model.status).unwrap_or(TaskStatus::Pending),
            attempts: model.attempts,
            max_attempts: model.max_attempts,
            error: model.error,
            scheduled_for: model.scheduled_for,
            created_at: model.created_at,
            updated_at: model.updated_at,
            started_at: model.started_at,
            completed_at: model.completed_at,
        })
    }

    async fn find_pending(&self, limit: usize) -> Result<Vec<TaskRecord>, TaskError> {
        let models = Entity::find()
            .filter(Column::Status.eq(TaskStatus::Pending.as_str()))
            .filter(
                Condition::any()
                    .add(Column::ScheduledFor.is_null())
                    .add(Column::ScheduledFor.lte(Utc::now())),
            )
            .order_by_asc(Column::CreatedAt)
            .limit(limit as u64)
            .all(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?;

        Ok(models
            .into_iter()
            .map(|m| TaskRecord {
                id: m.id.to_string(),
                task_type: m.task_type,
                payload: m.payload.clone(),
                status: TaskStatus::parse_status(&m.status).unwrap_or(TaskStatus::Pending),
                attempts: m.attempts,
                max_attempts: m.max_attempts,
                error: m.error,
                scheduled_for: m.scheduled_for,
                created_at: m.created_at,
                updated_at: m.updated_at,
                started_at: m.started_at,
                completed_at: m.completed_at,
            })
            .collect())
    }

    async fn mark_processing(&self, id: &str) -> Result<TaskRecord, TaskError> {
        let id_int: i32 = id
            .parse()
            .map_err(|_| TaskError::Storage("Invalid task ID".to_string()))?;

        let model = Entity::find_by_id(id_int)
            .one(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?
            .ok_or(TaskError::NotFound)?;

        let mut active: ActiveModel = model.into();
        active.status = Set(TaskStatus::Processing.as_str().to_string());
        active.started_at = Set(Some(Utc::now()));
        active.attempts = Set(active.attempts.unwrap() + 1);
        active.updated_at = Set(Utc::now());

        let updated = active
            .update(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?;

        Ok(TaskRecord {
            id: updated.id.to_string(),
            task_type: updated.task_type,
            payload: updated.payload.clone(),
            status: TaskStatus::parse_status(&updated.status).unwrap_or(TaskStatus::Processing),
            attempts: updated.attempts,
            max_attempts: updated.max_attempts,
            error: updated.error,
            scheduled_for: updated.scheduled_for,
            created_at: updated.created_at,
            updated_at: updated.updated_at,
            started_at: updated.started_at,
            completed_at: updated.completed_at,
        })
    }

    async fn mark_completed(&self, id: &str) -> Result<TaskRecord, TaskError> {
        let id_int: i32 = id
            .parse()
            .map_err(|_| TaskError::Storage("Invalid task ID".to_string()))?;

        let model = Entity::find_by_id(id_int)
            .one(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?
            .ok_or(TaskError::NotFound)?;

        let mut active: ActiveModel = model.into();
        active.status = Set(TaskStatus::Completed.as_str().to_string());
        active.completed_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());

        let updated = active
            .update(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?;

        Ok(TaskRecord {
            id: updated.id.to_string(),
            task_type: updated.task_type,
            payload: updated.payload.clone(),
            status: TaskStatus::parse_status(&updated.status).unwrap_or(TaskStatus::Completed),
            attempts: updated.attempts,
            max_attempts: updated.max_attempts,
            error: updated.error,
            scheduled_for: updated.scheduled_for,
            created_at: updated.created_at,
            updated_at: updated.updated_at,
            started_at: updated.started_at,
            completed_at: updated.completed_at,
        })
    }

    async fn mark_failed(&self, id: &str, error: String) -> Result<TaskRecord, TaskError> {
        let id_int: i32 = id
            .parse()
            .map_err(|_| TaskError::Storage("Invalid task ID".to_string()))?;

        let model = Entity::find_by_id(id_int)
            .one(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?
            .ok_or(TaskError::NotFound)?;

        let mut active: ActiveModel = model.clone().into();
        active.error = Set(Some(error));
        active.updated_at = Set(Utc::now());

        // If max attempts reached, mark as failed, otherwise mark as pending for retry
        if model.attempts >= model.max_attempts {
            active.status = Set(TaskStatus::Failed.as_str().to_string());
        } else {
            active.status = Set(TaskStatus::Pending.as_str().to_string());
        }

        let updated = active
            .update(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?;

        Ok(TaskRecord {
            id: updated.id.to_string(),
            task_type: updated.task_type,
            payload: updated.payload.clone(),
            status: TaskStatus::parse_status(&updated.status).unwrap_or(TaskStatus::Failed),
            attempts: updated.attempts,
            max_attempts: updated.max_attempts,
            error: updated.error,
            scheduled_for: updated.scheduled_for,
            created_at: updated.created_at,
            updated_at: updated.updated_at,
            started_at: updated.started_at,
            completed_at: updated.completed_at,
        })
    }

    async fn get_task(&self, id: &str) -> Result<Option<TaskRecord>, TaskError> {
        let id_int: i32 = id
            .parse()
            .map_err(|_| TaskError::Storage("Invalid task ID".to_string()))?;

        let model = Entity::find_by_id(id_int)
            .one(&self.db)
            .await
            .map_err(|e| TaskError::Storage(e.to_string()))?;

        Ok(model.map(|m| TaskRecord {
            id: m.id.to_string(),
            task_type: m.task_type,
            payload: m.payload.clone(),
            status: TaskStatus::parse_status(&m.status).unwrap_or(TaskStatus::Pending),
            attempts: m.attempts,
            max_attempts: m.max_attempts,
            error: m.error,
            scheduled_for: m.scheduled_for,
            created_at: m.created_at,
            updated_at: m.updated_at,
            started_at: m.started_at,
            completed_at: m.completed_at,
        }))
    }
}
