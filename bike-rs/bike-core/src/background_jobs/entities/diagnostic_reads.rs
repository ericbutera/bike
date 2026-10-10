use super::background_tasks::{Column, Entity, Model};
use chrono::{DateTime, Utc};
use sea_orm::{
    DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult, QuerySelect, Select,
    Statement,
};
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
    /// A bounded oldest sample for every eligible processor/state. These are
    /// diagnostic snapshots only: source payloads and results are never selected.
    pub async fn diagnostic_candidates(db: &DatabaseConnection) -> Result<Vec<Self>, DbErr> {
        let backend = db.get_database_backend();
        let (empty, parameter) = if backend == DbBackend::Postgres {
            ("jsonb_build_object()", "$1")
        } else {
            ("json_object()", "?")
        };
        let sql = format!(
            "SELECT id,task_type,{empty} AS payload,status,attempts,max_attempts,
      NULL AS error,NULL AS result,scheduled_for,created_at,updated_at,started_at,completed_at
    FROM (SELECT id,task_type,status,attempts,max_attempts,scheduled_for,
        created_at,updated_at,started_at,completed_at,
        ROW_NUMBER() OVER (PARTITION BY task_type,status ORDER BY updated_at,id) diagnostic_rank
      FROM background_tasks WHERE status IN ('pending','processing','failed')
        AND (status != 'pending' OR scheduled_for IS NULL OR scheduled_for <= {parameter})) ranked
    WHERE diagnostic_rank<=4"
        );
        Entity::find()
            .from_raw_sql(Statement::from_sql_and_values(
                backend,
                sql,
                [Utc::now().into()],
            ))
            .all(db)
            .await
    }
}
