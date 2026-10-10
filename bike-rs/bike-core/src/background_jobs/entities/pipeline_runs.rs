use sea_orm::entity::prelude::*;
use sea_orm::ConnectionTrait;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "processing_pipeline_runs")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub pipeline_started_at: DateTimeUtc,
    pub accepted_at: DateTimeUtc,
    pub entrypoint: String,
    pub request_id: Option<String>,
    pub trace_id: Option<String>,
    pub available_at: Option<DateTimeUtc>,
    pub gateway_history: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn record_task(
        db: &impl ConnectionTrait,
        context: &crate::background_jobs::pipeline::PipelineContext,
        task_id: i32,
    ) -> Result<(), DbErr> {
        use sea_orm::{sea_query::OnConflict, Set};
        Self::record_origin(db, context).await?;
        use sea_orm::QuerySelect;
        Entity::find_by_id(&context.run_id)
            .lock_exclusive()
            .one(db)
            .await?;
        super::pipeline_tasks::Entity::insert(super::pipeline_tasks::ActiveModel {
            run_id: Set(context.run_id.clone()),
            task_id: Set(task_id),
            parent_task_id: Set(context.parent_task_id),
        })
        .on_conflict(
            OnConflict::columns([
                super::pipeline_tasks::Column::RunId,
                super::pipeline_tasks::Column::TaskId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }

    pub async fn record_origin(
        db: &impl ConnectionTrait,
        context: &crate::background_jobs::pipeline::PipelineContext,
    ) -> Result<(), DbErr> {
        use sea_orm::{sea_query::OnConflict, Set};
        Entity::insert(ActiveModel {
            id: Set(context.run_id.clone()),
            pipeline_started_at: Set(context.pipeline_started_at),
            accepted_at: Set(chrono::Utc::now()),
            entrypoint: Set(context.entrypoint.clone()),
            request_id: Set(context.request_id.clone()),
            trace_id: Set(crate::observability::trace_id_from_carrier(
                context.trace_context.as_ref(),
            )),
            available_at: Set(None),
            gateway_history: Set(context.gateway_history.clone()),
        })
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
}
