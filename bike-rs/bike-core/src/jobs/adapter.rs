use crate::bike_tasks::{ActivityTask, ArchiveInput, FitnessChange, Priority, SegmentChange, UserPage};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use temporalio_client::Client;
use tokio::sync::OnceCell;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QueuedJobReference {
    pub id: String,
    pub status: String,
}

/// Domain producers submit directly to Temporal. There is no SQL queue.
#[derive(Clone)]
pub struct JobQueue {
    db: DatabaseConnection,
    client: Arc<OnceCell<Client>>,
}

impl JobQueue {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            client: Default::default(),
        }
    }

    pub fn with_client(db: DatabaseConnection, client: Client) -> Self {
        Self {
            db,
            client: Arc::new(OnceCell::new_with(Some(client))),
        }
    }

    async fn submit(&self, name: &str, input: &impl Serialize) -> Result<QueuedJobReference, String> {
        let input = serde_json::to_value(input).map_err(|error| error.to_string())?;
        let client = self
            .client
            .get_or_try_init(super::native::connect)
            .await
            .map_err(|error| error.to_string())?;
        super::native::submit(client, name, input)
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn rebuild_fitness_freshness(&self, user_id: i32) -> Result<(), super::TaskError> {
        self.submit(
            "rebuild_fitness_freshness",
            &FitnessChange {
                user_id,
                priority: Priority::Background,
            },
        )
        .await
        .map(|_| ())
        .map_err(super::TaskError::Storage)
    }

    pub async fn queue_fitness_freshness(&self, user_id: i32) -> Result<(), super::TaskError> {
        self.rebuild_fitness_freshness(user_id).await
    }

    pub async fn prepare_heatmap(&self, user_id: i32, activity_id: i32) -> Result<(), super::TaskError> {
        self.submit(
            "prepare_heatmap",
            &ActivityTask {
                user_id,
                activity_id,
                priority: Priority::NewActivity,
            },
        )
        .await
        .map(|_| ())
        .map_err(super::TaskError::Storage)
    }

    pub async fn rebuild_segment_analytics(&self, mut segment_ids: Vec<i32>) -> Result<(), super::TaskError> {
        segment_ids.retain(|id| *id > 0);
        segment_ids.sort_unstable();
        segment_ids.dedup();
        for segment_id in segment_ids {
            self.submit(
                "rebuild_segment_analytics",
                &SegmentChange {
                    segment_id,
                    priority: Priority::Background,
                },
            )
            .await
            .map_err(super::TaskError::Storage)?;
        }
        Ok(())
    }

    pub async fn regenerate_segment_efforts(&self, segment_id: i32) -> Result<QueuedJobReference, String> {
        if segment_id <= 0 {
            return Err("Segment ID must be positive".into());
        }
        self.submit(
            "regenerate_segment_efforts",
            &SegmentChange {
                segment_id,
                priority: Priority::Background,
            },
        )
        .await
    }

    pub async fn process_activity_import(&self, user_id: i32, import_id: i32) -> Result<(), String> {
        let source = crate::activity_reprocessing::import_source(
            &self.db,
            user_id,
            import_id,
            uuid::Uuid::new_v4().to_string(),
            Priority::NewActivity,
        )
        .await
        .map_err(|error| error.message)?;
        self.submit("process_activity_import", &source).await.map(|_| ())
    }

    async fn user_page(&self, name: &str, user_id: i32) -> Result<(), String> {
        if user_id <= 0 {
            return Err("User ID must be positive".into());
        }
        self.submit(name, &UserPage { user_id, after_id: 0 }).await.map(|_| ())
    }

    pub async fn regenerate_user_segments(&self, user_id: i32) -> Result<(), String> {
        self.user_page("regenerate_user_segments", user_id).await
    }

    pub async fn reprocess_user_activity_imports(&self, user_id: i32) -> Result<(), String> {
        self.user_page("reprocess_user_activity_imports", user_id).await
    }

    pub async fn reprocess_archive_fit_activity_imports(&self, user_id: i32) -> Result<(), String> {
        self.user_page("reprocess_archive_fit_activity_imports", user_id).await
    }

    pub async fn reprocess_activity_import(&self, activity_id: i32) -> Result<QueuedJobReference, String> {
        let input = crate::activity_reprocessing::command(
            &self.db,
            activity_id,
            uuid::Uuid::new_v4().to_string(),
            Priority::Background,
        )
        .await
        .map_err(|error| error.message)?;
        self.submit("reprocess_activity_import", &input).await
    }

    pub async fn backfill_user_xc_training(&self, user_id: i32) -> Result<(), String> {
        self.user_page("backfill_user_xc_training", user_id).await
    }

    pub async fn archive_activity_import(&self, job_id: i32) -> Result<(), String> {
        if job_id <= 0 {
            return Err("Archive ID must be positive".into());
        }
        self.submit("activity_archive_import", &ArchiveInput { job_id, cursor: None })
            .await
            .map(|_| ())
    }
}
