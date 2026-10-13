use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    Condition, DatabaseConnection, DbErr, ExprTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait,
};
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
    pub async fn active_strava_workflow_ids(
        db: &impl ConnectionTrait,
    ) -> Result<std::collections::BTreeSet<String>, DbErr> {
        let payloads: Vec<Json> = Entity::find()
            .select_only()
            .column(Column::Payload)
            .filter(Column::TaskType.is_in(Self::strava_workflow_task_types()))
            .filter(Column::Status.is_in(["pending", "processing"]))
            .into_tuple()
            .all(db)
            .await?;
        Ok(payloads
            .iter()
            .filter_map(|payload| payload["data"]["workflow_id"].as_str().map(str::to_owned))
            .collect())
    }

    fn strava_workflow_task_types() -> Vec<&'static str> {
        crate::strava::workflow::Action::ALL
            .into_iter()
            .map(|action| action.task_type())
            .chain(std::iter::once("strava_workflow"))
            .collect()
    }

    pub async fn latest_strava_workflow_task(
        db: &impl ConnectionTrait,
        workflow_id: &str,
    ) -> Result<Option<Self>, DbErr> {
        use sea_orm::sea_query::{Alias, Expr, Func};
        let payload = Column::Payload.into_expr();
        let workflow: Expr = match db.get_database_backend() {
            sea_orm::DatabaseBackend::Postgres => Func::cust(Alias::new("jsonb_extract_path_text"))
                .args([payload, Expr::val("data"), Expr::val("workflow_id")])
                .into(),
            sea_orm::DatabaseBackend::MySql => Func::cust(Alias::new("JSON_UNQUOTE"))
                .arg(
                    Func::cust(Alias::new("JSON_EXTRACT"))
                        .args([payload, Expr::val("$.data.workflow_id")]),
                )
                .into(),
            sea_orm::DatabaseBackend::Sqlite => Func::cust(Alias::new("json_extract"))
                .args([payload, Expr::val("$.data.workflow_id")])
                .into(),
            _ => {
                return Err(DbErr::Custom(
                    "Unsupported database for Strava task lookup".into(),
                ))
            }
        };
        Entity::find()
            .filter(Column::TaskType.is_in(Self::strava_workflow_task_types()))
            .filter(workflow.eq(workflow_id))
            .order_by_desc(Column::Id)
            .one(db)
            .await
    }

    pub fn with_correlation(query: sea_orm::Select<Entity>, id: &str) -> sea_orm::Select<Entity> {
        use super::{pipeline_runs, pipeline_tasks, task_attempts};
        let runs = pipeline_runs::Entity::find()
            .select_only()
            .column(pipeline_runs::Column::Id)
            .filter(
                Condition::any()
                    .add(pipeline_runs::Column::Id.eq(id))
                    .add(pipeline_runs::Column::RequestId.eq(id))
                    .add(pipeline_runs::Column::TraceId.eq(id)),
            )
            .into_query();
        let tasks = pipeline_tasks::Entity::find()
            .select_only()
            .column(pipeline_tasks::Column::TaskId)
            .filter(pipeline_tasks::Column::RunId.in_subquery(runs))
            .into_query();
        let traces = task_attempts::Entity::find()
            .select_only()
            .column(task_attempts::Column::TaskId)
            .filter(task_attempts::Column::TraceId.eq(id))
            .into_query();
        let mut matches = Condition::any()
            .add(Column::Id.in_subquery(tasks))
            .add(Column::Id.in_subquery(traces));
        if let Ok(task_id) = id.parse::<i32>() {
            matches = matches.add(Column::Id.eq(task_id));
        }
        query.filter(matches)
    }

    pub async fn has_live_activity_import_executor(
        db: &DatabaseConnection,
        import: &crate::entities::activity_imports::Model,
        stale_before: DateTime<Utc>,
    ) -> Result<bool, DbErr> {
        let tasks = Entity::find()
            .filter(Column::Status.eq(TaskStatus::Processing.as_str()))
            .filter(Column::UpdatedAt.gt(stale_before))
            .filter(Column::TaskType.is_in([
                "process_activity_import",
                "activity_archive_import",
                "reprocess_activity_import",
                "reprocess_user_activity_imports",
                "reprocess_archive_fit_activity_imports",
            ]))
            .all(db)
            .await?;
        Ok(tasks
            .iter()
            .any(|task| task.targets_activity_import(import)))
    }

    fn targets_activity_import(&self, import: &crate::entities::activity_imports::Model) -> bool {
        let data = self.payload.get("data").unwrap_or(&self.payload);
        let user_matches = data["user_id"].as_i64() == Some(i64::from(import.user_id));
        match self.task_type.as_str() {
            "process_activity_import" => {
                user_matches && data["import_id"].as_i64() == Some(i64::from(import.id))
            }
            "activity_archive_import" => import
                .archive_job_id
                .is_some_and(|id| data["job_id"].as_i64() == Some(i64::from(id))),
            "reprocess_activity_import" => import
                .activity_id
                .is_some_and(|id| data["activity_id"].as_i64() == Some(i64::from(id))),
            "reprocess_user_activity_imports" | "reprocess_archive_fit_activity_imports" => {
                user_matches
            }
            _ => false,
        }
    }

    /// Find pending tasks ready to be processed
    pub async fn find_pending(db: &DatabaseConnection, limit: u64) -> Result<Vec<Self>, DbErr> {
        Entity::find()
            .filter(Column::Status.eq(TaskStatus::Pending.as_str()))
            .filter(
                sea_orm::sea_query::Expr::col(Column::Attempts)
                    .lt(sea_orm::sea_query::Expr::col(Column::MaxAttempts)),
            )
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
}
