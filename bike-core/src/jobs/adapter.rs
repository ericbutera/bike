use crate::observability::{self, TraceContextCarrier};
use chrono::Utc;
use kaleido::auth::worker::tasks::{
    enqueue_email_notification, EmailNotificationTask, EMAIL_NOTIFICATION_TASK_TYPE,
};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use tracing::field;
use tracing::Instrument;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedJobReference {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Job {
    EmailNotification(EmailNotificationTask),
    RebuildFitnessFreshness(RebuildFitnessFreshnessTask),
    RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask),
    RegenerateSegmentEfforts(RegenerateSegmentEffortsTask),
    ProcessActivityImport(ProcessActivityImportTask),
    ReprocessUserActivityImports(ReprocessUserActivityImportsTask),
    ReprocessActivityImport(ReprocessActivityImportTask),
    BackfillUserXcTraining(BackfillUserXcTrainingTask),
    RegenerateUserSegments(RegenerateUserSegmentsTask),
    ActivityArchiveImport(ActivityArchiveImportTask),
    StravaSync(StravaSyncTask),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildFitnessFreshnessTask {
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildSegmentAnalyticsTask {
    pub segment_ids: Vec<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerateSegmentEffortsTask {
    pub segment_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessActivityImportTask {
    pub user_id: i32,
    pub import_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessUserActivityImportsTask {
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReprocessActivityImportTask {
    pub activity_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillUserXcTrainingTask {
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerateUserSegmentsTask {
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityArchiveImportTask {
    pub job_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StravaSyncTask {
    pub connection_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_context: Option<TraceContextCarrier>,
}

impl Job {
    pub fn task_type(&self) -> &'static str {
        match self {
            Job::EmailNotification(_) => EMAIL_NOTIFICATION_TASK_TYPE,
            Job::RebuildFitnessFreshness(_) => "rebuild_fitness_freshness",
            Job::RebuildSegmentAnalytics(_) => "rebuild_segment_analytics",
            Job::RegenerateSegmentEfforts(_) => "regenerate_segment_efforts",
            Job::ProcessActivityImport(_) => "process_activity_import",
            Job::ReprocessUserActivityImports(_) => "reprocess_user_activity_imports",
            Job::ReprocessActivityImport(_) => "reprocess_activity_import",
            Job::BackfillUserXcTraining(_) => "backfill_user_xc_training",
            Job::RegenerateUserSegments(_) => "regenerate_user_segments",
            Job::ActivityArchiveImport(_) => "activity_archive_import",
            Job::StravaSync(_) => "strava_sync",
        }
    }
}

#[derive(Clone)]
pub struct JobQueue {
    auth: kaleido::auth::AuthTaskQueue,
}

impl JobQueue {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            auth: kaleido::auth::AuthTaskQueue::new(db),
        }
    }

    pub fn auth_queue(&self) -> &kaleido::auth::AuthTaskQueue {
        &self.auth
    }

    async fn enqueue_job(&self, job: Job) {
        let task_type = job.task_type().to_string();
        let _ = self.auth.inner().enqueue(task_type, job).await;
    }

    async fn enqueue_job_with_options(
        &self,
        job: Job,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) {
        let task_type = job.task_type().to_string();
        let _ = self
            .auth
            .inner()
            .enqueue_with_options(task_type, job, scheduled_for, max_attempts)
            .await;
    }

    pub async fn email_notification(&self, to: String, subject: String, message: String) {
        enqueue_email_notification(self.auth.inner(), to, subject, message).await;
    }

    pub async fn rebuild_fitness_freshness(&self, user_id: i32) {
        self.enqueue_job(Job::RebuildFitnessFreshness(RebuildFitnessFreshnessTask {
            user_id,
        }))
        .await;
    }

    pub async fn rebuild_segment_analytics(&self, segment_ids: Vec<i32>) {
        let mut segment_ids = segment_ids
            .into_iter()
            .filter(|segment_id| *segment_id > 0)
            .collect::<Vec<_>>();
        segment_ids.sort_unstable();
        segment_ids.dedup();

        if segment_ids.is_empty() {
            return;
        }

        self.enqueue_job(Job::RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask {
            segment_ids,
        }))
        .await;
    }

    pub async fn regenerate_segment_efforts(
        &self,
        segment_id: i32,
    ) -> Result<QueuedJobReference, String> {
        if segment_id <= 0 {
            return Err("segment id must be positive".to_string());
        }

        let job = Job::RegenerateSegmentEfforts(RegenerateSegmentEffortsTask { segment_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|job| QueuedJobReference {
                id: job.id,
                status: job.status.as_str().to_string(),
            })
            .map_err(|error| error.to_string())
    }

    pub async fn process_activity_import(
        &self,
        user_id: i32,
        import_id: i32,
    ) -> Result<(), String> {
        if user_id <= 0 || import_id <= 0 {
            return Ok(());
        }

        let job = Job::ProcessActivityImport(ProcessActivityImportTask { user_id, import_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn regenerate_user_segments(&self, user_id: i32) -> Result<(), String> {
        let job = Job::RegenerateUserSegments(RegenerateUserSegmentsTask { user_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn reprocess_user_activity_imports(&self, user_id: i32) -> Result<(), String> {
        let job = Job::ReprocessUserActivityImports(ReprocessUserActivityImportsTask { user_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn reprocess_activity_import(
        &self,
        activity_id: i32,
    ) -> Result<QueuedJobReference, String> {
        let job = Job::ReprocessActivityImport(ReprocessActivityImportTask { activity_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|job| QueuedJobReference {
                id: job.id,
                status: job.status.as_str().to_string(),
            })
            .map_err(|error| error.to_string())
    }

    pub async fn backfill_user_xc_training(&self, user_id: i32) -> Result<(), String> {
        self.backfill_user_xc_training_with_options(user_id, None, 3)
            .await
    }

    pub async fn backfill_user_xc_training_with_options(
        &self,
        user_id: i32,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) -> Result<(), String> {
        let job = Job::BackfillUserXcTraining(BackfillUserXcTrainingTask { user_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, scheduled_for, max_attempts)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn archive_activity_import(&self, job_id: i32) -> Result<(), String> {
        let job = Job::ActivityArchiveImport(ActivityArchiveImportTask { job_id });
        let task_type = job.task_type().to_string();

        self.auth
            .inner()
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn sync_strava_connection(&self, connection_id: i32) -> Result<(), String> {
        self.sync_strava_connection_with_options(connection_id, None, 3)
            .await
    }

    pub async fn sync_strava_connection_with_options(
        &self,
        connection_id: i32,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) -> Result<(), String> {
        let span = tracing::info_span!(
            "task.enqueue",
            task_type = "strava_sync",
            connection_id,
            scheduled_for = scheduled_for.map(|value| value.to_rfc3339()),
            trace_id = field::Empty,
            span_id = field::Empty,
        );

        async move {
            observability::record_current_trace_context();
            let job = Job::StravaSync(StravaSyncTask {
                connection_id,
                trace_context: observability::inject_current_trace_context(),
            });
            let task_type = job.task_type().to_string();

            self.auth
                .inner()
                .enqueue_with_options(task_type, job, scheduled_for, max_attempts)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        .instrument(span)
        .await
    }

    pub async fn enqueue(&self, job: Job) {
        self.enqueue_with_options(job, None, 3).await
    }

    pub async fn enqueue_with_options(
        &self,
        job: Job,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) {
        self.enqueue_job_with_options(job, scheduled_for, max_attempts)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bike_jobs_serialize_for_worker_payload_contract() {
        let jobs = [
            (
                Job::RebuildFitnessFreshness(RebuildFitnessFreshnessTask { user_id: 12 }),
                "rebuild_fitness_freshness",
                json!({"type": "RebuildFitnessFreshness", "data": {"user_id": 12}}),
            ),
            (
                Job::RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask {
                    segment_ids: vec![1, 2],
                }),
                "rebuild_segment_analytics",
                json!({"type": "RebuildSegmentAnalytics", "data": {"segment_ids": [1, 2]}}),
            ),
            (
                Job::RegenerateSegmentEfforts(RegenerateSegmentEffortsTask { segment_id: 51 }),
                "regenerate_segment_efforts",
                json!({"type": "RegenerateSegmentEfforts", "data": {"segment_id": 51}}),
            ),
            (
                Job::ProcessActivityImport(ProcessActivityImportTask {
                    user_id: 12,
                    import_id: 34,
                }),
                "process_activity_import",
                json!({"type": "ProcessActivityImport", "data": {"user_id": 12, "import_id": 34}}),
            ),
            (
                Job::ReprocessUserActivityImports(ReprocessUserActivityImportsTask { user_id: 12 }),
                "reprocess_user_activity_imports",
                json!({"type": "ReprocessUserActivityImports", "data": {"user_id": 12}}),
            ),
            (
                Job::ReprocessActivityImport(ReprocessActivityImportTask { activity_id: 34 }),
                "reprocess_activity_import",
                json!({"type": "ReprocessActivityImport", "data": {"activity_id": 34}}),
            ),
            (
                Job::BackfillUserXcTraining(BackfillUserXcTrainingTask { user_id: 12 }),
                "backfill_user_xc_training",
                json!({"type": "BackfillUserXcTraining", "data": {"user_id": 12}}),
            ),
            (
                Job::RegenerateUserSegments(RegenerateUserSegmentsTask { user_id: 12 }),
                "regenerate_user_segments",
                json!({"type": "RegenerateUserSegments", "data": {"user_id": 12}}),
            ),
            (
                Job::ActivityArchiveImport(ActivityArchiveImportTask { job_id: 77 }),
                "activity_archive_import",
                json!({"type": "ActivityArchiveImport", "data": {"job_id": 77}}),
            ),
            (
                Job::StravaSync(StravaSyncTask {
                    connection_id: 99,
                    trace_context: None,
                }),
                "strava_sync",
                json!({"type": "StravaSync", "data": {"connection_id": 99}}),
            ),
        ];

        for (job, task_type, expected_payload) in jobs {
            assert_eq!(job.task_type(), task_type);
            assert_eq!(
                serde_json::to_value(job).expect("serialize job"),
                expected_payload
            );
        }
    }
}
