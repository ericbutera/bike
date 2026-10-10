use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "worker_batch_tasks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub batch_id: i32,
    #[sea_orm(primary_key, auto_increment = false)]
    pub task_id: i32,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
