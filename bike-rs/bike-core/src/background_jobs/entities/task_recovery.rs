use super::background_tasks::{Column, Entity, Model};
use chrono::{DateTime, Duration, Utc};
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

const STALE_HEARTBEAT_SECONDS: i64 = 300;

impl Model {
    /// Every processor uses the same heartbeat lease. Recover a bounded oldest
    /// page; successful recovery removes it from subsequent candidate scans.
    pub async fn recover_stale_executions(
        db: &DatabaseConnection,
        now: DateTime<Utc>,
        limit: u64,
    ) -> Result<u64, DbErr> {
        let candidates = Entity::find()
            .filter(Column::Status.eq("processing"))
            .filter(Column::UpdatedAt.lte(now - Duration::seconds(STALE_HEARTBEAT_SECONDS)))
            .order_by_asc(Column::UpdatedAt)
            .order_by_asc(Column::Id)
            .limit(limit)
            .all(db)
            .await?;
        let mut recovered = 0;
        for task in candidates {
            // recover compares status, attempt and updated_at atomically. A
            // heartbeat since this query keeps the current executor's lease.
            if task.recover(db, task.payload.clone()).await? {
                recovered += 1;
            }
        }
        Ok(recovered)
    }
}
