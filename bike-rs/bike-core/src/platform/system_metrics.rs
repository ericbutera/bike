use crate::entities::{activities, activity_imports, segments};
use crate::platform::aggregator::StoredRecordCounts;
use crate::platform::auth_metrics::AuthMetrics;
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr};

/// One database-backed snapshot for Prometheus; a failed query fails the snapshot.
#[derive(Debug)]
pub struct SystemMetrics {
    pub auth: AuthMetrics,
    pub activities: StoredRecordCounts,
    pub activity_imports: StoredRecordCounts,
    pub segments: StoredRecordCounts,
}

impl SystemMetrics {
    pub async fn collect(db: &DatabaseConnection) -> Result<Self, DbErr> {
        let now = Utc::now();
        let (auth, activities, activity_imports, segments) = tokio::try_join!(
            AuthMetrics::fetch(db, now),
            activities::Model::metrics_counts(db, now),
            activity_imports::Entity::metrics_counts(db, now),
            segments::Model::metrics_counts(db, now),
        )?;
        Ok(Self {
            auth,
            activities,
            activity_imports,
            segments,
        })
    }
}
