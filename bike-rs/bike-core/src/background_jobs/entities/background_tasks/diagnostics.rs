use super::*;
use chrono::Utc;
use sea_orm::{ColumnTrait, QueryFilter, QuerySelect, QueryTrait};
impl Model {
    pub async fn retired_history_tasks(
        db: &DatabaseConnection,
        cutoff: chrono::DateTime<Utc>,
    ) -> Result<Vec<i32>, DbErr> {
        let closed = Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::CompletedAt.lt(cutoff))
            .filter(Column::Status.is_in(["completed", "canceled"]))
            .filter(
                Column::Id.in_subquery(
                    crate::background_jobs::entities::task_attempts::Entity::find()
                        .select_only()
                        .column(crate::background_jobs::entities::task_attempts::Column::TaskId)
                        .into_query(),
                ),
            )
            .filter(
                Column::Id.not_in_subquery(
                    crate::background_jobs::entities::pipeline_tasks::Entity::find()
                        .select_only()
                        .column(crate::background_jobs::entities::pipeline_tasks::Column::TaskId)
                        .into_query(),
                ),
            )
            .limit(128)
            .into_tuple::<i32>()
            .all(db)
            .await?;
        Ok(closed)
    }
    pub async fn ready_count(db: &DatabaseConnection) -> Result<u64, DbErr> {
        use sea_orm::{Condition, ExprTrait, PaginatorTrait};
        Entity::find()
            .filter(Column::Status.eq("pending"))
            .filter(Expr::col(Column::Attempts).lt(Expr::col(Column::MaxAttempts)))
            .filter(
                Condition::any()
                    .add(Column::ScheduledFor.is_null())
                    .add(Column::ScheduledFor.lte(Utc::now())),
            )
            .count(db)
            .await
    }
    pub async fn unfinished_in(db: &impl ConnectionTrait, ids: Vec<i32>) -> Result<bool, DbErr> {
        Ok(Entity::find()
            .select_only()
            .column(Column::Id)
            .filter(Column::Id.is_in(ids))
            .filter(Column::Status.ne("completed"))
            .into_tuple::<i32>()
            .one(db)
            .await?
            .is_some())
    }
}

impl Model {
    pub async fn find_by_id(db: &impl ConnectionTrait, id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }
    pub async fn metadata_for_batch(
        db: &impl ConnectionTrait,
        id: i32,
    ) -> Result<Vec<crate::background_jobs::entities::diagnostic_reads::TaskMetadata>, DbErr> {
        use crate::background_jobs::entities::batch_tasks;
        let ids = batch_tasks::Entity::find()
            .select_only()
            .column(batch_tasks::Column::TaskId)
            .filter(batch_tasks::Column::BatchId.eq(id))
            .into_query();
        Self::metadata_query()
            .filter(
                sea_orm::Condition::any()
                    .add(Column::Id.eq(id))
                    .add(Column::Id.in_subquery(ids)),
            )
            .into_model()
            .all(db)
            .await
    }
}
