use crate::background_jobs::{DurableStorage, TaskQueue};
use crate::jobs::email::{enqueue_email_notification, EmailNotificationTask};
use chrono::Utc;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedJobReference {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Job {
    PrepareHeatmap(PrepareHeatmapTask),
    EmailNotification(EmailNotificationTask),
    RebuildFitnessFreshness(RebuildFitnessFreshnessTask),
    RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask),
    RegenerateSegmentEfforts(RegenerateSegmentEffortsTask),
    ProcessActivityImport(ProcessActivityImportTask),
    ReprocessUserActivityImports(ReprocessUserActivityImportsTask),
    ReprocessArchiveFitActivityImports(ReprocessArchiveFitActivityImportsTask),
    ReprocessActivityImport(ReprocessActivityImportTask),
    BackfillUserXcTraining(BackfillUserXcTrainingTask),
    RegenerateUserSegments(RegenerateUserSegmentsTask),
    RegenerateActivitySegments(RegenerateActivitySegmentsTask),
    ActivityArchiveImport(ActivityArchiveImportTask),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildFitnessFreshnessTask {
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareHeatmapTask {
    pub activities: Vec<PrepareHeatmapActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareHeatmapActivity {
    pub activity_id: i32,
    pub generation: i64,
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
pub struct ReprocessArchiveFitActivityImportsTask {
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
pub struct RegenerateActivitySegmentsTask {
    pub activity_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityArchiveImportTask {
    pub job_id: i32,
}

impl Job {
    pub fn kind(&self) -> super::JobType {
        match self {
            Self::PrepareHeatmap(_) => super::JobType::PrepareHeatmap,
            Self::EmailNotification(_) => super::JobType::EmailNotification,
            Self::RebuildFitnessFreshness(_) => super::JobType::RebuildFitnessFreshness,
            Self::RebuildSegmentAnalytics(_) => super::JobType::RebuildSegmentAnalytics,
            Self::RegenerateSegmentEfforts(_) => super::JobType::RegenerateSegmentEfforts,
            Self::ProcessActivityImport(_) => super::JobType::ProcessActivityImport,
            Self::ReprocessUserActivityImports(_) => super::JobType::ReprocessUserActivityImports,
            Self::ReprocessArchiveFitActivityImports(_) => {
                super::JobType::ReprocessArchiveFitActivityImports
            }
            Self::ReprocessActivityImport(_) => super::JobType::ReprocessActivityImport,
            Self::BackfillUserXcTraining(_) => super::JobType::BackfillUserXcTraining,
            Self::RegenerateUserSegments(_) => super::JobType::RegenerateUserSegments,
            Self::RegenerateActivitySegments(_) => super::JobType::RegenerateActivitySegments,
            Self::ActivityArchiveImport(_) => super::JobType::ActivityArchiveImport,
        }
    }

    pub fn task_type(&self) -> &'static str {
        self.kind().as_str()
    }
}

#[derive(Clone)]
pub struct JobQueue {
    queue: TaskQueue<DurableStorage>,
}

impl JobQueue {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            queue: TaskQueue::new(DurableStorage::new(db)),
        }
    }

    async fn enqueue_job(&self, job: Job) -> Result<(), crate::background_jobs::TaskError> {
        let task_type = job.task_type().to_string();
        self.queue.enqueue(task_type, job).await.map(|_| ())
    }

    pub async fn email_notification(
        &self,
        to: String,
        subject: String,
        message: String,
    ) -> Result<(), crate::background_jobs::TaskError> {
        enqueue_email_notification(&self.queue, to, subject, message).await
    }

    pub async fn rebuild_fitness_freshness(
        &self,
        user_id: i32,
    ) -> Result<(), crate::background_jobs::TaskError> {
        self.enqueue_job(Job::RebuildFitnessFreshness(RebuildFitnessFreshnessTask {
            user_id,
        }))
        .await
    }

    pub async fn queue_fitness_freshness(
        &self,
        user_id: i32,
    ) -> Result<(), crate::background_jobs::TaskError> {
        let job = Job::RebuildFitnessFreshness(RebuildFitnessFreshnessTask { user_id });
        self.enqueue_job(job).await
    }

    pub async fn rebuild_segment_analytics(
        &self,
        segment_ids: Vec<i32>,
    ) -> Result<(), crate::background_jobs::TaskError> {
        let mut segment_ids = segment_ids
            .into_iter()
            .filter(|segment_id| *segment_id > 0)
            .collect::<Vec<_>>();
        segment_ids.sort_unstable();
        segment_ids.dedup();

        if segment_ids.is_empty() {
            return Ok(());
        }

        self.enqueue_job(Job::RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask {
            segment_ids,
        }))
        .await
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

        self.queue
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

        self.process_activity_import_with_attempt(user_id, import_id, None)
            .await
    }

    pub async fn process_activity_import_with_attempt(
        &self,
        user_id: i32,
        import_id: i32,
        attempt_id: Option<i32>,
    ) -> Result<(), String> {
        let mut data = serde_json::json!({"user_id": user_id, "import_id": import_id});
        if let Some(id) = attempt_id {
            data["attempt_id"] = id.into();
        }
        self.queue
            .enqueue_with_options(
                super::JobType::ProcessActivityImport.as_str().into(),
                serde_json::json!({"data": data}),
                None,
                1,
            )
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn regenerate_user_segments(&self, user_id: i32) -> Result<(), String> {
        let job = Job::RegenerateUserSegments(RegenerateUserSegmentsTask { user_id });
        let task_type = job.task_type().to_string();

        self.queue
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn reprocess_user_activity_imports(&self, user_id: i32) -> Result<(), String> {
        self.enqueue_user_import_reprocessing(Job::ReprocessUserActivityImports(
            ReprocessUserActivityImportsTask { user_id },
        ))
        .await
    }

    pub async fn reprocess_archive_fit_activity_imports(&self, user_id: i32) -> Result<(), String> {
        self.enqueue_user_import_reprocessing(Job::ReprocessArchiveFitActivityImports(
            ReprocessArchiveFitActivityImportsTask { user_id },
        ))
        .await
    }

    async fn enqueue_user_import_reprocessing(&self, job: Job) -> Result<(), String> {
        let task_type = job.task_type().to_string();

        self.queue
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

        self.queue
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

        self.queue
            .enqueue_with_options(task_type, job, scheduled_for, max_attempts)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    pub async fn archive_activity_import(&self, job_id: i32) -> Result<(), String> {
        let job = Job::ActivityArchiveImport(ActivityArchiveImportTask { job_id });
        let task_type = job.task_type().to_string();

        self.queue
            .enqueue_with_options(task_type, job, None, 1)
            .await
            .map(|_| ())
            .map_err(|error| error.to_string())
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
        ];

        for (job, task_type, expected_payload) in jobs.into_iter().chain(additional_job_cases()) {
            assert_eq!(job.task_type(), task_type);
            assert_eq!(
                serde_json::to_value(job).expect("serialize job"),
                expected_payload
            );
        }
    }

    fn additional_job_cases() -> [(Job, &'static str, serde_json::Value); 4] {
        [
            (
                Job::PrepareHeatmap(PrepareHeatmapTask { activities: vec![] }),
                "prepare_heatmap",
                json!({"type": "PrepareHeatmap", "data": {"activities": []}}),
            ),
            (
                Job::EmailNotification(EmailNotificationTask {
                    to: "fixture@example.test".into(),
                    subject: "Fixture".into(),
                    message: "Fixture message".into(),
                }),
                "email_notification",
                json!({"type": "EmailNotification", "data": {"to": "fixture@example.test", "subject": "Fixture", "message": "Fixture message"}}),
            ),
            (
                Job::ReprocessArchiveFitActivityImports(ReprocessArchiveFitActivityImportsTask {
                    user_id: 12,
                }),
                "reprocess_archive_fit_activity_imports",
                json!({"type": "ReprocessArchiveFitActivityImports", "data": {"user_id": 12}}),
            ),
            (
                Job::RegenerateActivitySegments(RegenerateActivitySegmentsTask { activity_id: 34 }),
                "regenerate_activity_segments",
                json!({"type": "RegenerateActivitySegments", "data": {"activity_id": 34}}),
            ),
        ]
    }
}
