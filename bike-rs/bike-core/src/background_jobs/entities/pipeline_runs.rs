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

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PublicationState {
    pub pending_tasks: usize,
    pub pending_outputs: usize,
    pub failed_task: Option<i32>,
}

impl PublicationState {
    pub fn ready(&self) -> bool {
        self.pending_tasks == 0 && self.pending_outputs == 0 && self.failed_task.is_none()
    }
}

impl Model {
    pub async fn publication_state(
        db: &DatabaseConnection,
        run_id: &str,
    ) -> Result<PublicationState, DbErr> {
        use super::{background_tasks, pipeline_outputs, pipeline_tasks};
        use sea_orm::{
            AccessMode, ColumnTrait, IsolationLevel, QueryFilter, QuerySelect, TransactionTrait,
        };
        let snapshot = db
            .begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await?;
        let ids = pipeline_tasks::Entity::find()
            .select_only()
            .column(pipeline_tasks::Column::TaskId)
            .filter(pipeline_tasks::Column::RunId.eq(run_id))
            .into_tuple::<i32>()
            .all(&snapshot)
            .await?;
        let tasks = background_tasks::Entity::find()
            .select_only()
            .columns([
                background_tasks::Column::Id,
                background_tasks::Column::Status,
            ])
            .filter(background_tasks::Column::Id.is_in(ids.iter().copied()))
            .into_tuple::<(i32, String)>()
            .all(&snapshot)
            .await?;
        let outputs = pipeline_outputs::Entity::find()
            .filter(pipeline_outputs::Column::RunId.eq(run_id))
            .filter(pipeline_outputs::Column::Required.eq(true))
            .all(&snapshot)
            .await?;
        snapshot.commit().await?;
        Ok(PublicationState {
            pending_tasks: tasks
                .iter()
                .filter(|(_, status)| matches!(status.as_str(), "pending" | "processing"))
                .count(),
            pending_outputs: outputs
                .iter()
                .filter(|output| output.status != "available" || output.available_at.is_none())
                .count(),
            failed_task: failed_task(&ids, &tasks),
        })
    }

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

fn failed_task(ids: &[i32], tasks: &[(i32, String)]) -> Option<i32> {
    ids.iter()
        .find(|id| {
            !tasks.iter().any(|(task_id, status)| {
                task_id == *id && matches!(status.as_str(), "pending" | "processing" | "completed")
            })
        })
        .copied()
}

#[cfg(test)]
mod publication_tests {
    use super::*;

    #[test]
    fn missing_failed_canceled_and_unknown_tasks_block_publication() {
        for status in [None, Some("failed"), Some("canceled"), Some("unknown")] {
            let tasks = status
                .map(|status| vec![(42, status.into())])
                .unwrap_or_default();
            assert_eq!(failed_task(&[42], &tasks), Some(42));
        }
    }

    #[test]
    fn publication_requires_tasks_and_outputs_to_finish() {
        assert!(PublicationState::default().ready());
        assert!(!PublicationState {
            pending_tasks: 1,
            ..Default::default()
        }
        .ready());
        assert!(!PublicationState {
            pending_outputs: 1,
            ..Default::default()
        }
        .ready());
        assert!(!PublicationState {
            failed_task: Some(42),
            ..Default::default()
        }
        .ready());
        assert_eq!(failed_task(&[42], &[(42, "completed".into())]), None);
    }
}
