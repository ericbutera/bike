use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "background_task_attempts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub attempt: i32,
    pub started_at: DateTimeUtc,
    pub finished_at: Option<DateTimeUtc>,
    pub heartbeat_at: DateTimeUtc,
    pub progress_at: DateTimeUtc,
    pub outcome: String,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub eligible_at: Option<DateTimeUtc>,
    pub processing_version: Option<String>,
    pub workload_cohort: Option<String>,
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

impl Model {
    pub async fn progress(db: &impl ConnectionTrait, id: i32, attempt: i32) -> Result<(), DbErr> {
        use sea_orm::{sea_query::Expr, QueryFilter};
        Entity::update_many()
            .col_expr(Column::ProgressAt, Expr::value(chrono::Utc::now()))
            .filter(Column::TaskId.eq(id))
            .filter(Column::Attempt.eq(attempt))
            .filter(Column::Outcome.eq("running"))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn for_task(db: &DatabaseConnection, id: i32) -> Result<Vec<Self>, DbErr> {
        use sea_orm::{QueryFilter, QueryOrder};
        Entity::find()
            .filter(Column::TaskId.eq(id))
            .order_by_asc(Column::Attempt)
            .all(db)
            .await
    }
}
