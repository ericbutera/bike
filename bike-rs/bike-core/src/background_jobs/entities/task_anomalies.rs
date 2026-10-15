use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, serde::Serialize, utoipa::ToSchema)]
#[schema(as = WorkerAnomalyEvidence)]
#[sea_orm(table_name = "worker_task_anomalies")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub attempt: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub reason: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub policy_version: i32,
    pub observed: f64,
    pub expected: f64,
    pub baseline_samples: i64,
    #[schema(value_type = String, format = DateTime)]
    pub evaluated_at: DateTimeUtc,
    #[schema(value_type = Option<String>, format = DateTime)]
    pub resolved_at: Option<DateTimeUtc>,
    pub severity: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::background_tasks::Entity",
        from = "Column::TaskId",
        to = "super::background_tasks::Column::Id",
        on_delete = "Cascade"
    )]
    Task,
}
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
    pub async fn for_task(db: &impl ConnectionTrait, id: i32) -> Result<Vec<Self>, DbErr> {
        use sea_orm::QueryFilter;
        Entity::find().filter(Column::TaskId.eq(id)).all(db).await
    }
}
