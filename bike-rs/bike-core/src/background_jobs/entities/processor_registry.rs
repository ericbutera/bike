use crate::background_jobs::worker::catalog::ProcessorDefinition;
use chrono::Utc;
use sea_orm::{entity::prelude::*, sea_query::OnConflict, ConnectionTrait, Set};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "worker_processor_registry")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_type: String,
    pub definition: Json,
    pub observed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    pub async fn register(db: &impl ConnectionTrait, task_types: &[String]) -> Result<(), DbErr> {
        if let Ok(raw) = std::env::var("WORKER_PROCESSOR_BUDGETS") {
            crate::background_jobs::worker::catalog::validate_budget_json(&raw, task_types)?;
        }
        if task_types.is_empty() {
            return Ok(());
        }
        let models = task_types
            .iter()
            .map(|task_type| {
                Ok(ActiveModel {
                    task_type: Set(task_type.clone()),
                    definition: Set(
                        serde_json::to_value(ProcessorDefinition::for_type(task_type))
                            .map_err(|error| DbErr::Custom(error.to_string()))?,
                    ),
                    observed_at: Set(Utc::now()),
                })
            })
            .collect::<Result<Vec<_>, DbErr>>()?;
        Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(Column::TaskType)
                    .update_columns([Column::Definition, Column::ObservedAt])
                    .to_owned(),
            )
            .exec_without_returning(db)
            .await?;
        Ok(())
    }
}
