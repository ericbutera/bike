use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, FromQueryResult, PaginatorTrait,
    QueryFilter,
};

/// Counts of retained records, including the subset created in a rolling window.
#[derive(Debug, PartialEq, Eq)]
pub struct StoredRecordCounts {
    pub total: u64,
    pub created_last_30d: u64,
}

pub struct Aggregator;

impl Aggregator {
    pub async fn stored_records<E>(
        db: &DatabaseConnection,
        created_at: E::Column,
        now: DateTime<Utc>,
    ) -> Result<StoredRecordCounts, DbErr>
    where
        E: EntityTrait,
        E::Model: FromQueryResult + Send + Sync,
    {
        let (total, created_last_30d) = tokio::try_join!(
            E::find().count(db),
            Self::created_since::<E>(db, created_at, now - Duration::days(30)),
        )?;
        Ok(StoredRecordCounts {
            total,
            created_last_30d,
        })
    }

    pub async fn created_since<E>(
        db: &DatabaseConnection,
        created_at: E::Column,
        since: DateTime<Utc>,
    ) -> Result<u64, DbErr>
    where
        E: EntityTrait,
        E::Model: FromQueryResult + Send + Sync,
    {
        E::find().filter(created_at.gte(since)).count(db).await
    }
}
