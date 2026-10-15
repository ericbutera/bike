use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "processing_pipeline_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub run_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i32,
    pub parent_task_id: Option<i32>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::pipeline_runs::Entity",
        from = "Column::RunId",
        to = "super::pipeline_runs::Column::Id"
    )]
    PipelineRun,
}

impl Related<super::pipeline_runs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PipelineRun.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn origins(
        db: &DatabaseConnection,
        task_id: i32,
    ) -> Result<Vec<(Self, Option<super::pipeline_runs::Model>)>, DbErr> {
        use sea_orm::{QueryFilter, QueryOrder};
        Entity::find()
            .filter(Column::TaskId.eq(task_id))
            .order_by_asc(Column::RunId)
            .find_also_related(super::pipeline_runs::Entity)
            .all(db)
            .await
    }
}

impl Model {
    pub async fn delete_for_runs(db: &impl ConnectionTrait, ids: &[String]) -> Result<(), DbErr> {
        use sea_orm::QueryFilter;
        Entity::delete_many()
            .filter(Column::RunId.is_in(ids.iter().cloned()))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl Model {
    pub async fn task_ids(db: &impl ConnectionTrait, id: &str) -> Result<Vec<i32>, DbErr> {
        use sea_orm::{QueryFilter, QuerySelect};
        Entity::find()
            .filter(Column::RunId.eq(id))
            .select_only()
            .column(Column::TaskId)
            .into_tuple()
            .all(db)
            .await
    }
    pub async fn run_ids_for_task(
        db: &impl ConnectionTrait,
        id: i32,
    ) -> Result<Vec<String>, DbErr> {
        use sea_orm::{QueryFilter, QuerySelect};
        Entity::find()
            .filter(Column::TaskId.eq(id))
            .select_only()
            .column(Column::RunId)
            .into_tuple()
            .all(db)
            .await
    }
    pub async fn page_for_run(
        db: &impl ConnectionTrait,
        id: &str,
        after: i32,
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::{QueryFilter, QueryOrder, QuerySelect};
        Entity::find()
            .filter(Column::RunId.eq(id))
            .filter(Column::TaskId.gt(after))
            .order_by_asc(Column::TaskId)
            .limit(51)
            .all(db)
            .await
    }
}
