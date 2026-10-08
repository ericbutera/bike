use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{Condition, DatabaseConnection, DbErr, QueryFilter, QueryOrder, QuerySelect, Set};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    pub result: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl Model {
    /// Find pending tasks ready to be processed
    pub async fn find_pending(db: &DatabaseConnection, limit: u64) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(TaskStatus::Pending.as_str()))
            .filter(
                Condition::any()
                    .add(Column::ScheduledFor.is_null())
                    .add(Column::ScheduledFor.lte(Utc::now())),
            )
            .order_by_asc(Column::CreatedAt)
            .limit(limit)
            .all(db)
            .await
    }

    /// Mark task as processing
    pub async fn mark_processing(&self, db: &DatabaseConnection) -> Result<Model, DbErr> {
        let mut active: ActiveModel = self.clone().into();
        active.status = Set(TaskStatus::Processing.as_str().to_string());
        active.started_at = Set(Some(Utc::now()));
        active.attempts = Set(self.attempts + 1);
        active.updated_at = Set(Utc::now());
        active.update(db).await
    }

    /// Refresh updated_at for a task that is still actively processing.
    pub async fn mark_processing_heartbeat(
        db: &DatabaseConnection,
        id: i32,
    ) -> Result<Option<Model>, DbErr> {
        let Some(task) = Entity::find_by_id(id).one(db).await? else {
            return Ok(None);
        };

        if task.status != TaskStatus::Processing.as_str() {
            return Ok(Some(task));
        }

        let mut active: ActiveModel = task.into();
        active.updated_at = Set(Utc::now());
        active.update(db).await.map(Some)
    }

    /// Mark task as completed
    pub async fn mark_completed(&self, db: &DatabaseConnection) -> Result<Model, DbErr> {
        self.mark_completed_with_result(db, None).await
    }

    /// Mark task as completed with an optional result message
    pub async fn mark_completed_with_result(
        &self,
        db: &DatabaseConnection,
        result: Option<String>,
    ) -> Result<Model, DbErr> {
        let mut active: ActiveModel = self.clone().into();
        active.status = Set(TaskStatus::Completed.as_str().to_string());
        active.completed_at = Set(Some(Utc::now()));
        active.updated_at = Set(Utc::now());
        active.result = Set(result);
        active.update(db).await
    }

    /// Mark task as failed
    pub async fn mark_failed(
        &self,
        db: &DatabaseConnection,
        error: String,
    ) -> Result<Model, DbErr> {
        let mut active: ActiveModel = self.clone().into();
        active.error = Set(Some(error));
        active.updated_at = Set(Utc::now());

        // If max attempts reached, mark as failed permanently
        if self.attempts >= self.max_attempts {
            active.status = Set(TaskStatus::Failed.as_str().to_string());
        } else {
            // Otherwise, set back to pending for retry
            active.status = Set(TaskStatus::Pending.as_str().to_string());
        }

        active.update(db).await
    }
}
