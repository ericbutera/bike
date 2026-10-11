use sea_orm::entity::prelude::*;
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "worker_batches")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i32,
    pub user_id: i32,
    pub kind: String,
    pub source: Json,
    pub cursor: i32,
    pub sealed: bool,
    pub status: String,
    pub updated_at: DateTimeUtc,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}

impl Model {
    /// Keyset pages visit every running batch, including those behind batches
    /// whose descendants are still active. IDs are stable across reconciliation.
    pub async fn running_page(
        db: &DatabaseConnection,
        after_id: i32,
        limit: u64,
    ) -> Result<Vec<Self>, DbErr> {
        use sea_orm::{QueryOrder, QuerySelect};
        Entity::find()
            .filter(Column::Status.eq("running"))
            .filter(Column::Id.gt(after_id))
            .order_by_asc(Column::Id)
            .limit(limit)
            .all(db)
            .await
    }
}
