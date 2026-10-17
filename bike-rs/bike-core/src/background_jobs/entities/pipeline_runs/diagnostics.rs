use super::*;
use crate::background_jobs::entities::{background_tasks, pipeline_outputs, pipeline_tasks};
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait};
impl Model {
    pub async fn ready_candidates(db: &DatabaseConnection) -> Result<Vec<Self>, DbErr> {
        let outputs = pipeline_outputs::Entity::find()
            .select_only()
            .column(pipeline_outputs::Column::RunId)
            .into_query();
        let pending = pipeline_outputs::Entity::find()
            .select_only()
            .column(pipeline_outputs::Column::RunId)
            .filter(pipeline_outputs::Column::Required.eq(true))
            .filter(pipeline_outputs::Column::AvailableAt.is_null())
            .into_query();
        let unfinished = background_tasks::Entity::find()
            .select_only()
            .column(background_tasks::Column::Id)
            .filter(background_tasks::Column::Status.ne("completed"))
            .into_query();
        let open_trees = pipeline_tasks::Entity::find()
            .select_only()
            .column(pipeline_tasks::Column::RunId)
            .filter(pipeline_tasks::Column::TaskId.in_subquery(unfinished))
            .into_query();
        let candidates = Entity::find()
            .filter(Column::AvailableAt.is_null())
            .filter(Column::Id.in_subquery(outputs))
            .filter(Column::Id.not_in_subquery(pending))
            .filter(Column::Id.not_in_subquery(open_trees))
            .order_by_asc(Column::AcceptedAt)
            .limit(128)
            .all(db)
            .await?;
        Ok(candidates)
    }
    pub async fn expired_roots(
        db: &DatabaseConnection,
        cutoff: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<String>, DbErr> {
        let roots = Entity::find()
            .filter(
                sea_orm::Condition::any().add(Column::AvailableAt.lt(cutoff)).add(
                    sea_orm::Condition::all()
                        .add(Column::AcceptedAt.lt(cutoff))
                        .add(
                            Column::Id.not_in_subquery(
                                pipeline_outputs::Entity::find()
                                    .select_only()
                                    .column(pipeline_outputs::Column::RunId)
                                    .filter(pipeline_outputs::Column::AvailableAt.is_null())
                                    .into_query(),
                            ),
                        )
                        .add(
                            Column::Id.not_in_subquery(
                                pipeline_tasks::Entity::find()
                                    .select_only()
                                    .column(pipeline_tasks::Column::RunId)
                                    .filter(
                                        pipeline_tasks::Column::TaskId.in_subquery(
                                            background_tasks::Entity::find()
                                                .select_only()
                                                .column(background_tasks::Column::Id)
                                                .filter(
                                                    background_tasks::Column::Status.is_in(["pending", "processing"]),
                                                )
                                                .into_query(),
                                        ),
                                    )
                                    .into_query(),
                            ),
                        ),
                ),
            )
            .select_only()
            .column(Column::Id)
            .limit(128)
            .into_tuple::<String>()
            .all(db)
            .await?;
        Ok(roots)
    }
    pub async fn find_by_id(db: &impl ConnectionTrait, id: &str) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }
    pub async fn lock(db: &impl ConnectionTrait, id: &str) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).lock_exclusive().one(db).await
    }
    pub async fn make_available(
        &self,
        db: &impl ConnectionTrait,
        available_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<bool, DbErr> {
        Ok(Entity::update_many()
            .col_expr(Column::AvailableAt, sea_orm::sea_query::Expr::value(available_at))
            .filter(Column::Id.eq(self.id.clone()))
            .filter(Column::AvailableAt.is_null())
            .exec(db)
            .await?
            .rows_affected
            > 0)
    }
}

impl Model {
    pub async fn oldest_unready(
        db: &DatabaseConnection,
        entrypoint: &str,
    ) -> Result<Option<chrono::DateTime<chrono::Utc>>, DbErr> {
        let timestamp = Entity::find()
            .filter(Column::Entrypoint.eq(entrypoint))
            .filter(Column::AvailableAt.is_null())
            .filter(
                Column::Id.in_subquery(
                    pipeline_outputs::Entity::find()
                        .select_only()
                        .column(pipeline_outputs::Column::RunId)
                        .filter(pipeline_outputs::Column::Required.eq(true))
                        .filter(pipeline_outputs::Column::AvailableAt.is_null())
                        .into_query(),
                ),
            )
            .select_only()
            .column(Column::PipelineStartedAt)
            .order_by_asc(Column::PipelineStartedAt)
            .limit(1)
            .into_tuple::<chrono::DateTime<chrono::Utc>>()
            .one(db)
            .await?;
        Ok(timestamp)
    }
}
