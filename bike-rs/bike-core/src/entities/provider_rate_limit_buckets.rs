use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbBackend, DbErr, QueryFilter, QueryOrder, QuerySelect, Set,
};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "provider_rate_limit_buckets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub provider: String,
    pub bucket: String,
    pub limit_count: i32,
    pub used_count: i32,
    pub reset_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now();
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);
        Ok(self)
    }
}

impl Model {
    pub async fn list_ordered<C>(db: &C) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .order_by_asc(Column::Provider)
            .order_by_asc(Column::Bucket)
            .all(db)
            .await
    }

    pub async fn list_for_reservation<C>(
        db: &C,
        provider: &str,
        bucket_names: &[&str],
    ) -> Result<Vec<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        let mut query = Entity::find()
            .filter(Column::Provider.eq(provider))
            .filter(Column::Bucket.is_in(bucket_names.iter().copied()));

        if db.get_database_backend() == DbBackend::Postgres {
            query = query.lock_exclusive();
        }

        query.all(db).await
    }

    pub async fn find_bucket<C>(
        db: &C,
        provider: &str,
        bucket: &str,
    ) -> Result<Option<Model>, DbErr>
    where
        C: ConnectionTrait,
    {
        Entity::find()
            .filter(Column::Provider.eq(provider))
            .filter(Column::Bucket.eq(bucket))
            .one(db)
            .await
    }
}
