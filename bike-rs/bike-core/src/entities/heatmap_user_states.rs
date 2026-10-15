use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "heatmap_user_states")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: i32,
    pub revision: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

use sea_orm::sea_query::Expr;
use sea_orm::{ConnectionTrait, DbErr, ExprTrait, QueryFilter, QuerySelect};
impl Model {
    pub async fn bump_revision(db: &impl ConnectionTrait, user_id: i32) -> Result<(), DbErr> {
        Entity::update_many()
            .col_expr(Column::Revision, Expr::col(Column::Revision).add(1))
            .filter(Column::UserId.eq(user_id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn revision(db: &impl ConnectionTrait, user_id: i32) -> Result<i64, DbErr> {
        Ok(Entity::find_by_id(user_id)
            .select_only()
            .column(Column::Revision)
            .into_tuple::<i64>()
            .one(db)
            .await?
            .unwrap_or(0))
    }
}
