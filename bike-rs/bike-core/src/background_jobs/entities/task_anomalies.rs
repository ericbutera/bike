use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, utoipa::ToSchema)]
#[schema(as = WorkerAnomalyEvidence)]
#[sea_orm(table_name = "worker_task_anomalies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub attempt: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub reason: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub policy_version: i32,
    pub observed: f64,
    pub expected: f64,
    pub baseline_samples: i64,
    #[schema(value_type = String, format = DateTime)]
    pub evaluated_at: DateTimeUtc,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub resolved_at: Option<DateTimeUtc>,
    pub severity: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::background_tasks::Entity",
        from = "Column::TaskId",
        to = "super::background_tasks::Column::Id",
        on_delete = "Cascade"
    )]
    Task,
}
impl ActiveModelBehavior for ActiveModel {}

#[derive(sea_orm::FromQueryResult)]
pub struct ActiveCount {
    pub task_type: String,
    pub reason: String,
    pub count: i64,
}
impl Model {
    /// Cancelled/closed or deliberately scheduled work must stop contributing
    /// operational alerts even when it is outside the bounded sampler page.
    pub async fn resolve_inapplicable(db: &impl ConnectionTrait) -> Result<(), DbErr> {
        use super::background_tasks;
        use sea_orm::{sea_query::Expr, Condition, QuerySelect, QueryTrait};
        let now = chrono::Utc::now();
        let groups: &[(&[&str], &str)] = &[
            (&["queue_budget"], "pending"),
            (
                &["runtime_budget", "heartbeat_stale", "progress_stale"],
                "processing",
            ),
            (&["retry_exhausted"], "failed"),
        ];
        for (reasons, status) in groups {
            let mut applicable = background_tasks::Entity::find()
                .select_only()
                .column(background_tasks::Column::Id)
                .filter(background_tasks::Column::Status.eq(*status));
            if *status == "pending" {
                applicable = applicable.filter(
                    Condition::any()
                        .add(background_tasks::Column::ScheduledFor.is_null())
                        .add(background_tasks::Column::ScheduledFor.lte(now)),
                );
            }
            Entity::update_many()
                .col_expr(Column::ResolvedAt, Expr::value(now))
                .filter(Column::ResolvedAt.is_null())
                .filter(Column::Reason.is_in(reasons.iter().copied()))
                .filter(Column::TaskId.not_in_subquery(applicable.into_query()))
                .exec(db)
                .await?;
        }
        Ok(())
    }

    pub async fn active_counts(db: &impl ConnectionTrait) -> Result<Vec<ActiveCount>, DbErr> {
        use sea_orm::{JoinType, QuerySelect};
        Entity::find()
            .select_only()
            .column(super::background_tasks::Column::TaskType)
            .column(Column::Reason)
            .column_as(Column::TaskId.count(), "count")
            .join(JoinType::InnerJoin, Relation::Task.def())
            .filter(Column::ResolvedAt.is_null())
            .group_by(super::background_tasks::Column::TaskType)
            .group_by(Column::Reason)
            .into_model::<ActiveCount>()
            .all(db)
            .await
    }
}
