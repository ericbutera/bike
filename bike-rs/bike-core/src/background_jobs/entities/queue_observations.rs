//! Aggregate snapshots keep monitoring work bounded by processor types, not queue size.
use super::background_tasks::{self, Column, Entity, Model};
use chrono::{DateTime, Utc};
use sea_orm::{
    sea_query::{Alias, Expr, Func},
    ColumnTrait, Condition, DatabaseConnection, DbErr, EntityTrait, ExprTrait, FromQueryResult,
    QueryFilter, QuerySelect,
};

#[derive(FromQueryResult)]
pub struct StateCount {
    pub task_type: String,
    pub state: String,
    pub count: i64,
}
#[derive(FromQueryResult)]
pub struct QueueAge {
    pub task_type: String,
    pub eligible_at: Option<DateTime<Utc>>,
}

impl Model {
    pub async fn state_counts(db: &DatabaseConnection) -> Result<Vec<StateCount>, DbErr> {
        let now = Utc::now();
        let state = Expr::case(Column::Status.eq("processing"), "running")
            .case(
                Column::Status
                    .eq("pending")
                    .and(Column::ScheduledFor.gt(now)),
                "scheduled",
            )
            .case(
                Column::Status.eq("pending").and(Column::Attempts.gt(0)),
                "retrying",
            )
            .case(Column::Status.eq("pending"), "queued")
            .finally(Expr::col(Column::Status));
        Entity::find()
            .select_only()
            .column(Column::TaskType)
            .column_as(Expr::Case(Box::new(state)), "state")
            .column_as(Column::Id.count(), "count")
            .group_by(Column::TaskType)
            .group_by(Expr::col(Alias::new("state")))
            .filter(
                Condition::any()
                    .add(Column::Status.ne("completed"))
                    .add(Column::CompletedAt.gte(now - chrono::Duration::hours(24))),
            )
            .into_model::<StateCount>()
            .all(db)
            .await
    }
    pub async fn eligible_ages(db: &DatabaseConnection) -> Result<Vec<QueueAge>, DbErr> {
        let eligible = Expr::Case(Box::new(
            Expr::case(
                Expr::col(Column::ScheduledFor).gt(Expr::col(Column::UpdatedAt)),
                Expr::col(Column::ScheduledFor),
            )
            .finally(Expr::col(Column::UpdatedAt)),
        ));
        Entity::find()
            .select_only()
            .column(Column::TaskType)
            .column_as(
                sea_orm::sea_query::SimpleExpr::from(Func::min(eligible)),
                "eligible_at",
            )
            .filter(Column::Status.eq("pending"))
            .filter(Expr::col(Column::Attempts).lt(Expr::col(Column::MaxAttempts)))
            .filter(
                Condition::any()
                    .add(Column::ScheduledFor.is_null())
                    .add(Column::ScheduledFor.lte(Utc::now())),
            )
            .group_by(Column::TaskType)
            .into_model::<QueueAge>()
            .all(db)
            .await
    }
}

use super::task_attempts;
#[derive(FromQueryResult)]
pub struct ExecutionAge {
    pub task_type: String,
    pub runtime: Option<DateTime<Utc>>,
    pub heartbeat: Option<DateTime<Utc>>,
    pub progress: Option<DateTime<Utc>>,
}
impl task_attempts::Model {
    pub async fn active_ages(db: &DatabaseConnection) -> Result<Vec<ExecutionAge>, DbErr> {
        use sea_orm::{JoinType, RelationTrait};
        task_attempts::Entity::find()
            .select_only()
            .column(background_tasks::Column::TaskType)
            .column_as(task_attempts::Column::StartedAt.min(), "runtime")
            .column_as(task_attempts::Column::HeartbeatAt.min(), "heartbeat")
            .column_as(task_attempts::Column::ProgressAt.min(), "progress")
            .join(JoinType::InnerJoin, task_attempts::Relation::Task.def())
            .filter(task_attempts::Column::Outcome.eq("running"))
            .group_by(background_tasks::Column::TaskType)
            .into_model::<ExecutionAge>()
            .all(db)
            .await
    }
}
