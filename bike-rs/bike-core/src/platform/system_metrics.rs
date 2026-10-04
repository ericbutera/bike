use crate::platform::aggregator::NamedStat;
use crate::platform::auth_metrics::AuthMetrics;
use crate::platform::background_task_metrics::BackgroundTaskMetrics;
use sea_orm::DatabaseConnection;
use serde::Serialize;
use utoipa::ToSchema;

/// System-wide metrics automatically aggregated from all glass subsystems.
///
/// Add new subsystem fields here (and populate them in `collect`) when new
/// system-level metric categories are added to glass. Consuming controllers
/// embed this via `#[serde(flatten)]` so new sections appear in the JSON
/// response without any controller changes.
#[derive(Debug, Serialize, ToSchema)]
pub struct SystemMetrics {
    pub auth: Vec<NamedStat>,
    pub background_tasks: Vec<NamedStat>,
}

impl SystemMetrics {
    pub async fn collect(db: &DatabaseConnection) -> Self {
        let (auth, background_tasks) =
            tokio::join!(AuthMetrics::fetch(db), BackgroundTaskMetrics::fetch(db),);
        Self {
            auth: auth.into_named_stats(),
            background_tasks,
        }
    }
}
