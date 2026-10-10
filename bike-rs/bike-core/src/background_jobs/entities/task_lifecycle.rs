use super::{
    background_tasks::{Column, Entity, Model, TaskStatus},
    task_attempts,
};
use chrono::Utc;
use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, DbErr,
    EntityTrait, ExprTrait, QueryFilter, Set, TransactionTrait,
};

impl Model {
    pub fn eligible_at(&self) -> chrono::DateTime<Utc> {
        self.scheduled_for
            .map_or(self.updated_at, |time| std::cmp::max(time, self.updated_at))
    }
    /// Compare-and-swap the pending snapshot and record its execution atomically.
    pub async fn claim(&self, db: &DatabaseConnection) -> Result<Option<Self>, DbErr> {
        let transaction = db.begin().await?;
        let now = Utc::now();
        let changed = Entity::update_many()
            .col_expr(Column::Status, Expr::value("processing"))
            .col_expr(Column::StartedAt, Expr::value(now))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .col_expr(
                Column::CompletedAt,
                Expr::value(None::<chrono::DateTime<Utc>>),
            )
            .col_expr(Column::Attempts, Expr::value(self.attempts + 1))
            .filter(Column::Id.eq(self.id))
            .filter(Column::Status.eq("pending"))
            .filter(Column::Attempts.eq(self.attempts))
            .filter(Column::UpdatedAt.eq(self.updated_at))
            .filter(Expr::col(Column::Attempts).lt(Expr::col(Column::MaxAttempts)))
            .filter(
                Condition::any()
                    .add(Column::ScheduledFor.is_null())
                    .add(Column::ScheduledFor.lte(now)),
            )
            .exec(&transaction)
            .await?;
        if changed.rows_affected == 0 {
            return Ok(None);
        }
        task_attempts::ActiveModel {
            task_id: Set(self.id),
            attempt: Set(self.attempts + 1),
            started_at: Set(now),
            finished_at: Set(None),
            heartbeat_at: Set(now),
            progress_at: Set(now),
            outcome: Set("running".into()),
            error: Set(None),
            trace_id: Set(None),
            span_id: Set(None),
            eligible_at: Set(Some(self.eligible_at())),
            processing_version: Set(Some(
                super::super::worker::catalog::ProcessorDefinition::for_type(&self.task_type)
                    .processing_version,
            )),
            workload_cohort: Set(Some(super::super::worker::catalog::workload_cohort(
                &self.payload,
            ))),
        }
        .insert(&transaction)
        .await?;
        let task = Entity::find_by_id(self.id).one(&transaction).await?;
        transaction.commit().await?;
        Ok(task)
    }

    pub async fn mark_processing(&self, db: &DatabaseConnection) -> Result<Self, DbErr> {
        self.claim(db)
            .await?
            .ok_or_else(|| DbErr::Custom("Task was already claimed or is not eligible".into()))
    }

    pub async fn heartbeat(&self, db: &DatabaseConnection) -> Result<bool, DbErr> {
        let transaction = db.begin().await?;
        let now = Utc::now();
        let changed = self
            .execution_update()
            .filter(Column::Status.eq("processing"))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .exec(&transaction)
            .await?;
        if changed.rows_affected > 0 {
            task_attempts::Entity::update_many()
                .col_expr(task_attempts::Column::HeartbeatAt, Expr::value(now))
                .filter(task_attempts::Column::TaskId.eq(self.id))
                .filter(task_attempts::Column::Attempt.eq(self.attempts))
                .filter(task_attempts::Column::Outcome.eq("running"))
                .exec(&transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(changed.rows_affected > 0)
    }

    fn execution_update(&self) -> sea_orm::UpdateMany<Entity> {
        Entity::update_many()
            .filter(Column::Id.eq(self.id))
            .filter(Column::Status.eq(&self.status))
            .filter(Column::Attempts.eq(self.attempts))
    }

    pub async fn record_execution_trace(&self, db: &DatabaseConnection) -> Result<(), DbErr> {
        let Some((trace_id, span_id)) = crate::observability::current_trace_ids() else {
            return Ok(());
        };
        task_attempts::Entity::update_many()
            .col_expr(task_attempts::Column::TraceId, Expr::value(trace_id))
            .col_expr(task_attempts::Column::SpanId, Expr::value(span_id))
            .filter(task_attempts::Column::TaskId.eq(self.id))
            .filter(task_attempts::Column::Attempt.eq(self.attempts))
            .filter(task_attempts::Column::Outcome.eq("running"))
            .exec(db)
            .await?;
        Ok(())
    }

    /// Recovery keeps attempt numbers monotonic and cannot race a fresh heartbeat.
    pub async fn recover(
        &self,
        db: &DatabaseConnection,
        payload: serde_json::Value,
    ) -> Result<bool, DbErr> {
        let transaction = db.begin().await?;
        let now = Utc::now();
        let changed = self
            .execution_update()
            .filter(Column::Status.is_in(["pending", "processing"]))
            .filter(Column::UpdatedAt.eq(self.updated_at))
            .col_expr(Column::Status, Expr::value("pending"))
            .col_expr(Column::Payload, Expr::value(payload))
            .col_expr(
                Column::MaxAttempts,
                Expr::value(std::cmp::max(self.max_attempts, self.attempts + 1)),
            )
            .col_expr(Column::Error, Expr::value(None::<String>))
            .col_expr(Column::Result, Expr::value(None::<String>))
            .col_expr(
                Column::ScheduledFor,
                Expr::value(None::<chrono::DateTime<Utc>>),
            )
            .col_expr(
                Column::StartedAt,
                Expr::value(None::<chrono::DateTime<Utc>>),
            )
            .col_expr(
                Column::CompletedAt,
                Expr::value(None::<chrono::DateTime<Utc>>),
            )
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .exec(&transaction)
            .await?;
        if changed.rows_affected > 0 && self.status == "processing" {
            task_attempts::Entity::update_many()
                .col_expr(task_attempts::Column::FinishedAt, Expr::value(now))
                .col_expr(task_attempts::Column::Outcome, Expr::value("interrupted"))
                .col_expr(
                    task_attempts::Column::Error,
                    Expr::value("Recovered stale execution"),
                )
                .filter(task_attempts::Column::TaskId.eq(self.id))
                .filter(task_attempts::Column::Attempt.eq(self.attempts))
                .filter(task_attempts::Column::Outcome.eq("running"))
                .exec(&transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(changed.rows_affected > 0)
    }

    pub async fn mark_completed(&self, db: &DatabaseConnection) -> Result<Self, DbErr> {
        self.mark_completed_with_result(db, None).await
    }

    pub async fn mark_completed_with_result(
        &self,
        db: &DatabaseConnection,
        result: Option<String>,
    ) -> Result<Self, DbErr> {
        self.finish(db, TaskStatus::Completed, None, result)
            .await
            .map(|(task, _)| task)
    }

    pub async fn mark_failed(&self, db: &DatabaseConnection, error: String) -> Result<Self, DbErr> {
        self.finish(db, self.failure_status(), Some(error), None)
            .await
            .map(|(task, _)| task)
    }

    pub async fn cancel(&self, db: &DatabaseConnection) -> Result<Self, DbErr> {
        self.finish(
            db,
            TaskStatus::Canceled,
            Some("Canceled by admin".into()),
            None,
        )
        .await
        .map(|(task, _)| task)
    }

    fn failure_status(&self) -> TaskStatus {
        if self.attempts >= self.max_attempts {
            TaskStatus::Failed
        } else {
            TaskStatus::Pending
        }
    }

    /// None means this executor no longer owns the status/attempt it claimed.
    pub async fn finish_execution(
        &self,
        db: &DatabaseConnection,
        result: Result<(), String>,
    ) -> Result<Option<Self>, DbErr> {
        let (task, changed) = match result {
            Ok(()) => self.finish(db, TaskStatus::Completed, None, None).await?,
            Err(error) => {
                self.finish(db, self.failure_status(), Some(error), None)
                    .await?
            }
        };
        Ok(changed.then_some(task))
    }

    async fn finish(
        &self,
        db: &DatabaseConnection,
        status: TaskStatus,
        error: Option<String>,
        result: Option<String>,
    ) -> Result<(Self, bool), DbErr> {
        let transaction = db.begin().await?;
        let now = Utc::now();
        let terminal = status != TaskStatus::Pending;
        let changed = self
            .execution_update()
            .filter(Column::Status.is_in(["pending", "processing"]))
            .col_expr(Column::Status, Expr::value(status.as_str()))
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .col_expr(Column::CompletedAt, Expr::value(terminal.then_some(now)))
            .col_expr(Column::Error, Expr::value(error.clone()))
            .col_expr(Column::Result, Expr::value(result))
            .exec(&transaction)
            .await?;
        if changed.rows_affected > 0 && self.status == "processing" {
            task_attempts::Entity::update_many()
                .col_expr(task_attempts::Column::FinishedAt, Expr::value(now))
                .col_expr(
                    task_attempts::Column::Outcome,
                    Expr::value(if terminal {
                        status.as_str()
                    } else {
                        "retrying"
                    }),
                )
                .col_expr(task_attempts::Column::Error, Expr::value(error))
                .filter(task_attempts::Column::TaskId.eq(self.id))
                .filter(task_attempts::Column::Attempt.eq(self.attempts))
                .filter(task_attempts::Column::Outcome.eq("running"))
                .exec(&transaction)
                .await?;
        }
        let updated = Entity::find_by_id(self.id)
            .one(&transaction)
            .await?
            .ok_or_else(|| DbErr::Custom("Task no longer exists".into()))?;
        transaction.commit().await?;
        Ok((updated, changed.rows_affected > 0))
    }
}
