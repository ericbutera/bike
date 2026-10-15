use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, utoipa::ToSchema)]
#[schema(as = WorkerWorkEvidence)]
#[sea_orm(table_name = "processing_work_units")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub task_id: i32,
    pub attempt: i32,
    pub kind: String,
    pub work_key: String,
    pub revision: String,
    pub processing_version: String,
    pub mode: String,
    pub reason: String,
    #[schema(value_type = String, format = DateTime)]
    pub started_at: DateTimeUtc,
    #[schema(value_type = String, format = DateTime)]
    pub finished_at: DateTimeUtc,
    pub outcome: String,
    pub counts: Option<Json>,
    pub error: Option<String>,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn delete_for_tasks(db: &impl ConnectionTrait, ids: &[i32]) -> Result<(), DbErr> {
        use sea_orm::QueryFilter;
        Entity::delete_many()
            .filter(Column::TaskId.is_in(ids.iter().copied()))
            .exec(db)
            .await?;
        Ok(())
    }
}

impl Model {
    pub async fn page_for_task(
        db: &impl ConnectionTrait,
        id: i32,
        after: i32,
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::{QueryFilter, QueryOrder, QuerySelect};
        Entity::find()
            .filter(Column::TaskId.eq(id))
            .filter(Column::Id.gt(after))
            .order_by_asc(Column::Id)
            .limit(26)
            .all(db)
            .await
    }
}
