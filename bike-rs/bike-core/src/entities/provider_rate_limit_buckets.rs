use crate::entities::provider_rate_limit_buckets;
use crate::provider_rate_limit::ProviderQuotaBucketSpec;
use crate::workflow_error::WorkflowError as AppError;
use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use sea_orm::{ColumnTrait, ConnectionTrait, DbBackend, DbErr, QueryFilter, QueryOrder, QuerySelect, Set};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "provider_rate_limit_buckets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub provider: String,
    pub bucket: String,
    pub limit_count: i32,
    #[sea_orm(default_value = 0)]
    pub used_count: i32,
    pub reset_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub created_at: DateTime<Utc>,
    #[sea_orm(default_expr = "Expr::current_timestamp()")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::timestamps::impl_timestamps!(ActiveModel);

impl Model {
    pub fn effective_used_count(&self, now: DateTime<Utc>) -> i32 {
        if self.reset_at <= now {
            0
        } else {
            self.used_count
        }
    }

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

    pub async fn list_for_reservation<C>(db: &C, provider: &str, bucket_names: &[&str]) -> Result<Vec<Model>, DbErr>
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

    pub async fn find_bucket<C>(db: &C, provider: &str, bucket: &str) -> Result<Option<Model>, DbErr>
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

impl Model {
    pub async fn reserve(
        self,
        db: &impl ConnectionTrait,
        spec: &ProviderQuotaBucketSpec,
        now: DateTime<Utc>,
    ) -> Result<provider_rate_limit_buckets::Model, AppError> {
        let mut active_model: provider_rate_limit_buckets::ActiveModel = self.into();
        if active_model.reset_at.as_ref() <= &now {
            active_model.used_count = Set(spec.units);
            active_model.reset_at = Set(now + spec.window);
        } else {
            let used_count = *active_model.used_count.as_ref() + spec.units;
            active_model.used_count = Set(used_count);
        }
        active_model.limit_count = Set(spec.limit_count);
        Ok(active_model.update(db).await?)
    }
    pub async fn reconcile_usage(
        db: &DatabaseConnection,
        provider: &'static str,
        bucket: &'static str,
        limit_count: i32,
        used_count: i32,
        reset_at: DateTime<Utc>,
    ) -> Result<Option<Self>, AppError> {
        if limit_count <= 0 || used_count < 0 {
            return Ok(None);
        }

        let now = Utc::now();
        let Some(row) = provider_rate_limit_buckets::Model::find_bucket(db, provider, bucket).await? else {
            let row = provider_rate_limit_buckets::ActiveModel {
                provider: Set(provider.to_string()),
                bucket: Set(bucket.to_string()),
                limit_count: Set(limit_count),
                used_count: Set(used_count),
                reset_at: Set(reset_at),
                ..Default::default()
            };
            let row = row.insert(db).await?;
            return Ok(Some(row));
        };

        let should_reconcile = row.needs_reconciliation(now, limit_count, used_count);
        if !should_reconcile {
            return Ok(Some(row));
        }

        let mut active_model: provider_rate_limit_buckets::ActiveModel = row.into();
        active_model.limit_count = Set(limit_count);
        active_model.used_count = Set(used_count);
        active_model.reset_at = Set(reset_at);
        let updated = active_model.update(db).await?;
        Ok(Some(updated))
    }
    fn needs_reconciliation(&self, now: DateTime<Utc>, limit_count: i32, used_count: i32) -> bool {
        let usage_changed = used_count > self.used_count || limit_count != self.limit_count;
        self.reset_at <= now || usage_changed
    }
    pub async fn initialize(
        db: &impl ConnectionTrait,
        spec: &ProviderQuotaBucketSpec,
        now: DateTime<Utc>,
    ) -> Result<Self, DbErr> {
        provider_rate_limit_buckets::ActiveModel {
            provider: Set(spec.provider.to_string()),
            bucket: Set(spec.bucket.to_string()),
            limit_count: Set(spec.limit_count),
            reset_at: Set(now + spec.window),
            ..Default::default()
        }
        .insert(db)
        .await
    }
}
