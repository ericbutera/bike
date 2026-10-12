use super::background_tasks::{Column, Entity, Model};
use chrono::{DateTime, Utc};
use sea_orm::{EntityTrait, FromQueryResult, QuerySelect, Select};
#[derive(Debug, FromQueryResult)]
pub struct TaskMetadata {
    pub id: i32,
    pub task_type: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub scheduled_for: Option<DateTime<Utc>>,
}
impl Model {
    /// Graphs and barriers never load worker source payloads or results.
    pub fn metadata_query() -> Select<Entity> {
        Entity::find().select_only().columns([
            Column::Id,
            Column::TaskType,
            Column::Status,
            Column::Error,
            Column::CreatedAt,
            Column::ScheduledFor,
        ])
    }
}
