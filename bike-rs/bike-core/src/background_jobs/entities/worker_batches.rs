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

impl Model {
    pub async fn find_by_id(db: &impl ConnectionTrait, id: i32) -> Result<Option<Self>, DbErr> {
        Entity::find_by_id(id).one(db).await
    }
    pub async fn lock(db: &impl ConnectionTrait, id: i32) -> Result<Option<Self>, DbErr> {
        use sea_orm::QuerySelect;
        Entity::find_by_id(id).lock_exclusive().one(db).await
    }
}

impl Model {
    pub async fn create_if_absent(
        db: &impl ConnectionTrait,
        id: i32,
        user_id: i32,
        kind: &str,
        source: serde_json::Value,
    ) -> Result<(), DbErr> {
        use sea_orm::{sea_query::OnConflict, Set};
        Entity::insert(ActiveModel {
            id: Set(id),
            user_id: Set(user_id),
            kind: Set(kind.into()),
            source: Set(source),
            cursor: Set(0),
            sealed: Set(false),
            status: Set("running".into()),
            updated_at: Set(chrono::Utc::now()),
        })
        .on_conflict(OnConflict::column(Column::Id).do_nothing().to_owned())
        .exec_without_returning(db)
        .await?;
        Ok(())
    }
    pub async fn advance_page(
        &self,
        db: &impl ConnectionTrait,
        next: i32,
        sealed: bool,
    ) -> Result<(), DbErr> {
        use sea_orm::{sea_query::Expr, QueryFilter};
        Entity::update_many()
            .col_expr(Column::Cursor, Expr::value(next))
            .col_expr(Column::Sealed, Expr::value(sealed))
            .col_expr(Column::UpdatedAt, Expr::value(chrono::Utc::now()))
            .filter(Column::Id.eq(self.id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn finish(
        &self,
        db: &impl ConnectionTrait,
        failed: bool,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), DbErr> {
        use sea_orm::{sea_query::Expr, QueryFilter};
        Entity::update_many()
            .col_expr(
                Column::Status,
                Expr::value(if failed { "partial" } else { "completed" }),
            )
            .col_expr(Column::UpdatedAt, Expr::value(now))
            .filter(Column::Id.eq(self.id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn store_source(&self, db: &impl ConnectionTrait) -> Result<(), DbErr> {
        use sea_orm::{sea_query::Expr, QueryFilter};
        Entity::update_many()
            .col_expr(Column::Source, Expr::value(self.source.clone()))
            .filter(Column::Id.eq(self.id))
            .exec(db)
            .await?;
        Ok(())
    }
    pub async fn cleanup_candidates(db: &impl ConnectionTrait) -> Result<Vec<Self>, DbErr> {
        use sea_orm::{sea_query::Expr, QueryFilter, QueryOrder, QuerySelect};
        Entity::find()
            .filter(Column::Kind.eq("archive"))
            .filter(Column::Status.is_in(["completed", "partial"]))
            .filter(Expr::cust("source ->> 'archive_path' IS NOT NULL"))
            .order_by_asc(Column::UpdatedAt)
            .limit(64)
            .all(db)
            .await
    }
}
