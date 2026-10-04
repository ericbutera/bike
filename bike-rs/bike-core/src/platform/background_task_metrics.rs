use crate::background_jobs::entities::background_tasks;
use crate::platform::aggregator::NamedStat;
use chrono::{Duration, Utc};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, PaginatorTrait, QueryFilter};

pub struct BackgroundTaskMetrics;

impl BackgroundTaskMetrics {
    /// Fetches background task metrics concurrently.
    pub async fn fetch(db: &DatabaseConnection) -> Vec<NamedStat> {
        let f_pending = Self::count_by_status(db, "pending");
        let f_completed = Self::count_by_status_since(db, "completed", 24);
        let f_failed = Self::count_by_status_since(db, "failed", 24);

        let (pending, completed, failed) = tokio::join!(f_pending, f_completed, f_failed);

        vec![
            NamedStat::new("tasks_pending", "Pending Tasks", "current", pending),
            NamedStat::new(
                "tasks_completed_last_24h",
                "Completed (24h)",
                "last 24 hours",
                completed,
            ),
            NamedStat::new(
                "tasks_failed_last_24h",
                "Failed (24h)",
                "last 24 hours",
                failed,
            ),
        ]
    }

    async fn count_by_status(db: &DatabaseConnection, status: &str) -> Result<u64, DbErr> {
        background_tasks::Entity::find()
            .filter(background_tasks::Column::Status.eq(status.to_string()))
            .count(db)
            .await
    }

    async fn count_by_status_since(
        db: &DatabaseConnection,
        status: &str,
        hours: i64,
    ) -> Result<u64, DbErr> {
        let threshold = Utc::now() - Duration::hours(hours);
        background_tasks::Entity::find()
            .filter(background_tasks::Column::Status.eq(status.to_string()))
            .filter(background_tasks::Column::UpdatedAt.gte(threshold))
            .count(db)
            .await
    }
}
