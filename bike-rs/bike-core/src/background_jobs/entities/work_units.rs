use crate::background_jobs::entities::work_units;
use crate::background_jobs::execution::{ExecutionContext, WorkCounts, WorkScope};
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::Set;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, utoipa::ToSchema)]
#[schema(as = WorkerWorkEvidence)]
#[sea_orm(table_name = "processing_work_units")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub task_id: i32,
    pub attempt: i32,
    pub kind: String,
    pub work_key: String,
    pub revision: String,
    pub processing_version: String,
    pub mode: String,
    pub reason: String,
    #[schema(value_type = String, format = DateTime)]
    pub started_at: DateTimeUtc,
    #[schema(value_type = String, format = DateTime)]
    pub finished_at: DateTimeUtc,
    pub outcome: String,
    pub counts: Option<Json>,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn delete_for_tasks(db: &impl ConnectionTrait, ids: &[i32]) -> Result<(), DbErr> {
        use sea_orm::QueryFilter;
        Entity::delete_many()
            .filter(Column::TaskId.is_in(ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl Model {
    pub async fn record(
        db: &impl ConnectionTrait,
        context: &ExecutionContext,
        scope: &WorkScope,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        counts: Option<&WorkCounts>,
        error: Option<String>,
    ) -> Result<Self, DbErr> {
        let trace = crate::observability::current_trace_ids();
        let record = work_units::ActiveModel {
            task_id: Set(context.task_id),
            attempt: Set(context.attempt),
            kind: Set(scope.kind.into()),
            work_key: Set(scope.work_key.clone()),
            revision: Set(scope.revision.clone()),
            processing_version: Set(crate::background_jobs::worker::catalog::ProcessorDefinition::for_type(
                &context.task_type,
            )
            .processing_version),
            mode: Set(scope.mode.into()),
            reason: Set(scope.reason.into()),
            started_at: Set(started_at),
            finished_at: Set(finished_at),
            outcome: Set(counts.map(|counts| counts.outcome).unwrap_or("failed").into()),
            counts: Set(counts.and_then(|counts| serde_json::to_value(counts).ok())),
            error: Set(error),
            trace_id: Set(trace.as_ref().map(|ids| ids.0.clone())),
            span_id: Set(trace.map(|ids| ids.1)),
            ..Default::default()
        };

        record.insert(db).await
    }
}
