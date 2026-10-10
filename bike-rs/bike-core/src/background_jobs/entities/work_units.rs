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
