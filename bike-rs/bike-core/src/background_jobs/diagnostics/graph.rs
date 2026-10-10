use super::super::{
    entities::{
        background_tasks, pipeline_outputs, pipeline_runs, pipeline_subjects, pipeline_tasks,
        task_anomalies, work_units,
    },
    history::TaskAttemptResponse,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct PipelinePage {
    pub run_ids: Vec<String>,
    pub next_cursor: Option<String>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineTask {
    pub id: i32,
    pub task_type: String,
    pub status: String,
    pub parent_task_id: Option<i32>,
    pub created_at: String,
    pub scheduled_for: Option<String>,
    pub attempts: Vec<TaskAttemptResponse>,
    pub work: Vec<work_units::Model>,
    pub work_has_more: bool,
    pub next_work_cursor: Option<i32>,
    pub anomalies: Vec<task_anomalies::Model>,
    pub imports: Vec<PipelineImport>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineImport {
    pub attempt_id: i32,
    pub import_id: i32,
    pub activity_id: Option<i32>,
    pub status: String,
    pub stages: Vec<crate::activity_import_execution::StageRecord>,
    pub edges: Vec<PipelineStageEdge>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineStageEdge {
    pub from: String,
    pub to: String,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct PipelineGraph {
    pub run_id: String,
    pub entrypoint: String,
    pub received_at: String,
    pub accepted_at: String,
    pub available_at: Option<String>,
    pub ended_at: Option<String>,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub gateway_history: Option<serde_json::Value>,
    pub grafana_url: Option<String>,
    pub tasks: Vec<PipelineTask>,
    pub outputs: Vec<pipeline_outputs::Model>,
    pub next_task_cursor: Option<i32>,
    pub next_output_offset: Option<u64>,
}

impl PipelinePage {
    pub async fn for_activity(
        db: &DatabaseConnection,
        activity_id: i32,
        after: Option<String>,
    ) -> Result<Self, DbErr> {
        let query = pipeline_subjects::Entity::find()
            .filter(pipeline_subjects::Column::Kind.eq("activity"))
            .filter(pipeline_subjects::Column::SubjectId.eq(activity_id));
        let query = if let Some(after) = after {
            query.filter(pipeline_subjects::Column::RunId.gt(after))
        } else {
            query
        };
        let mut run_ids = query
            .select_only()
            .column(pipeline_subjects::Column::RunId)
            .order_by_asc(pipeline_subjects::Column::RunId)
            .limit(21)
            .into_tuple::<String>()
            .all(db)
            .await?;
        let next_cursor = if run_ids.len() > 20 {
            run_ids.truncate(20);
            run_ids.last().cloned()
        } else {
            None
        };
        Ok(Self {
            run_ids,
            next_cursor,
        })
    }
}

impl PipelineGraph {
    pub async fn load(
        db: &DatabaseConnection,
        id: &str,
        after_task: i32,
    ) -> Result<Option<Self>, DbErr> {
        Self::load_page(
            db,
            id,
            &GraphCursor {
                after_task: Some(after_task),
                ..Default::default()
            },
        )
        .await
    }
    pub async fn load_page(
        db: &DatabaseConnection,
        id: &str,
        cursor: &GraphCursor,
    ) -> Result<Option<Self>, DbErr> {
        let Some(run) = pipeline_runs::Entity::find_by_id(id).one(db).await? else {
            return Ok(None);
        };
        let mut links = pipeline_tasks::Entity::find()
            .filter(pipeline_tasks::Column::RunId.eq(id))
            .filter(pipeline_tasks::Column::TaskId.gt(cursor.after_task.unwrap_or(0)))
            .order_by_asc(pipeline_tasks::Column::TaskId)
            .limit(51)
            .all(db)
            .await?;
        let next_task_cursor = if links.len() > 50 {
            links.truncate(50);
            links.last().map(|link| link.task_id)
        } else {
            None
        };
        let mut tasks = Vec::new();
        for link in links {
            let Some(task) = background_tasks::Model::metadata_query()
                .filter(background_tasks::Column::Id.eq(link.task_id))
                .into_model::<super::super::entities::diagnostic_reads::TaskMetadata>()
                .one(db)
                .await?
            else {
                continue;
            };
            let work_after = if cursor.work_task == Some(task.id) {
                cursor.after_work.unwrap_or(0)
            } else {
                0
            };
            tasks.push(PipelineTask::load(db, task, link.parent_task_id, work_after).await?);
        }
        let (outputs, next_output_offset) =
            output_page(db, id, cursor.output_offset.unwrap_or(0)).await?;
        let ended_at = ended_at(db, id).await?;
        Ok(Some(Self {
            run_id: run.id,
            entrypoint: run.entrypoint,
            received_at: run.pipeline_started_at.to_rfc3339(),
            accepted_at: run.accepted_at.to_rfc3339(),
            available_at: run.available_at.map(|time| time.to_rfc3339()),
            ended_at,
            next_output_offset,
            request_id: run.request_id,
            trace_id: run.trace_id,
            gateway_history: run.gateway_history,
            grafana_url: crate::config::Config::get().grafana_url.clone(),
            tasks,
            outputs,
            next_task_cursor,
        }))
    }
}

impl PipelineTask {
    async fn load(
        db: &DatabaseConnection,
        task: super::super::entities::diagnostic_reads::TaskMetadata,
        parent_task_id: Option<i32>,
        work_after: i32,
    ) -> Result<Self, DbErr> {
        let mut work = work_units::Entity::find()
            .filter(work_units::Column::TaskId.eq(task.id))
            .filter(work_units::Column::Id.gt(work_after))
            .order_by_asc(work_units::Column::Id)
            .limit(26)
            .all(db)
            .await?;
        let work_has_more = work.len() > 25;
        work.truncate(25);
        let next_work_cursor = work_has_more
            .then(|| work.last().map(|work| work.id))
            .flatten();
        let anomalies = task_anomalies::Entity::find()
            .filter(task_anomalies::Column::TaskId.eq(task.id))
            .all(db)
            .await?;
        let imports =
            crate::entities::activity_import_attempts::Entity::for_worker_metadata(db, task.id)
                .await?
                .into_iter()
                .map(|attempt| {
                    let stages = crate::activity_import_execution::stages(&attempt)
                        .map_err(|error| DbErr::Custom(error.message))?;
                    Ok(PipelineImport {
                        attempt_id: attempt.id,
                        import_id: attempt.activity_import_id,
                        activity_id: attempt.activity_id,
                        status: attempt.status,
                        stages,
                        edges: crate::activity_import_pipeline::activity_processing_graph_nodes()
                            .iter()
                            .flat_map(|node| {
                                node.depends_on.iter().map(|dependency| PipelineStageEdge {
                                    from: dependency.id().into(),
                                    to: node.node.id().into(),
                                })
                            })
                            .collect(),
                    })
                })
                .collect::<Result<Vec<_>, DbErr>>()?;
        Ok(Self {
            id: task.id,
            task_type: task.task_type,
            status: task.status,
            parent_task_id,
            created_at: task.created_at.to_rfc3339(),
            scheduled_for: task.scheduled_for.map(|time| time.to_rfc3339()),
            attempts: super::super::entities::task_attempts::Model::for_task(db, task.id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
            work,
            work_has_more,
            next_work_cursor,
            anomalies,
            imports,
        })
    }
}

#[derive(Debug, Default, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in=Query)]
pub struct GraphCursor {
    pub after_task: Option<i32>,
    pub after_run: Option<String>,
    pub work_task: Option<i32>,
    pub after_work: Option<i32>,
    pub output_offset: Option<u64>,
}
async fn output_page(
    db: &DatabaseConnection,
    id: &str,
    offset: u64,
) -> Result<(Vec<pipeline_outputs::Model>, Option<u64>), DbErr> {
    let mut outputs = pipeline_outputs::Entity::find()
        .filter(pipeline_outputs::Column::RunId.eq(id))
        .order_by_asc(pipeline_outputs::Column::Kind)
        .order_by_asc(pipeline_outputs::Column::TargetId)
        .offset(offset)
        .limit(101)
        .all(db)
        .await?;
    let next = (outputs.len() > 100).then_some(offset + 100);
    outputs.truncate(100);
    Ok((outputs, next))
}
/// End is distinct from publication and unknown while any linked task is active.
async fn ended_at(db: &DatabaseConnection, id: &str) -> Result<Option<String>, DbErr> {
    let ids = pipeline_tasks::Entity::find()
        .select_only()
        .column(pipeline_tasks::Column::TaskId)
        .filter(pipeline_tasks::Column::RunId.eq(id))
        .into_query();
    let active = background_tasks::Entity::find()
        .select_only()
        .column(background_tasks::Column::Id)
        .filter(background_tasks::Column::Id.in_subquery(ids.clone()))
        .filter(background_tasks::Column::Status.is_in(["pending", "processing"]))
        .into_tuple::<i32>()
        .one(db)
        .await?;
    if active.is_some() {
        return Ok(None);
    }
    let end = background_tasks::Entity::find()
        .filter(background_tasks::Column::Id.in_subquery(ids))
        .select_only()
        .expr(sea_orm::sea_query::Func::max(
            sea_orm::sea_query::Func::coalesce([
                sea_orm::sea_query::Expr::col(background_tasks::Column::CompletedAt),
                sea_orm::sea_query::Expr::col(background_tasks::Column::UpdatedAt),
            ]),
        ))
        .into_tuple::<Option<chrono::DateTime<chrono::Utc>>>()
        .one(db)
        .await?
        .flatten();
    Ok(end.map(|end| end.to_rfc3339()))
}
use sea_orm::QueryTrait;
