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

impl Model {
    pub async fn attach(db: &impl ConnectionTrait, batch_id: i32, task_id: i32) -> Result<(), DbErr> {
        use sea_orm::{sea_query::OnConflict, Set};
        Entity::insert(ActiveModel {
            batch_id: Set(batch_id),
            task_id: Set(task_id),
        })
        .on_conflict(
            OnConflict::columns([Column::BatchId, Column::TaskId])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
}
