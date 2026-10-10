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
