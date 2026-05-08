use chrono::Utc;
use kaleido::auth::worker::tasks as auth_tasks;
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub use kaleido::auth::DefaultEnvAuthService as AppAuthService;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Task {
    EmailRegistration(auth_tasks::EmailRegistrationTask),
    EmailPasswordReset(auth_tasks::EmailPasswordResetTask),
    EmailNotification(auth_tasks::EmailNotificationTask),
    RebuildFitnessFreshness(RebuildFitnessFreshnessTask),
    RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildFitnessFreshnessTask {
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RebuildSegmentAnalyticsTask {
    pub segment_ids: Vec<i32>,
}

impl Task {
    pub fn task_type(&self) -> &'static str {
        match self {
            Task::EmailRegistration(_) => auth_tasks::EMAIL_REGISTRATION_TASK_TYPE,
            Task::EmailPasswordReset(_) => auth_tasks::EMAIL_PASSWORD_RESET_TASK_TYPE,
            Task::EmailNotification(_) => auth_tasks::EMAIL_NOTIFICATION_TASK_TYPE,
            Task::RebuildFitnessFreshness(_) => "rebuild_fitness_freshness",
            Task::RebuildSegmentAnalytics(_) => "rebuild_segment_analytics",
        }
    }
}

#[derive(Clone)]
pub struct TaskQueue {
    auth: kaleido::auth::AuthTaskQueue,
}

impl TaskQueue {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            auth: kaleido::auth::AuthTaskQueue::new(db),
        }
    }

    pub fn auth_queue(&self) -> &kaleido::auth::AuthTaskQueue {
        &self.auth
    }

    async fn enqueue_task(&self, task: Task) {
        let task_type = task.task_type().to_string();
        let _ = self.auth.inner().enqueue(task_type, task).await;
    }

    async fn enqueue_task_with_options(
        &self,
        task: Task,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) {
        let task_type = task.task_type().to_string();
        let _ = self
            .auth
            .inner()
            .enqueue_with_options(task_type, task, scheduled_for, max_attempts)
            .await;
    }

    pub async fn email_registration(&self, to: String, name: String, verification_url: String) {
        auth_tasks::enqueue_email_registration(self.auth.inner(), to, name, verification_url).await;
    }

    pub async fn email_password_reset(
        &self,
        to: String,
        name: String,
        reset_url: String,
        expiry_hours: u32,
    ) {
        auth_tasks::enqueue_email_password_reset(
            self.auth.inner(),
            to,
            name,
            reset_url,
            expiry_hours,
        )
        .await;
    }

    pub async fn email_notification(&self, to: String, subject: String, message: String) {
        auth_tasks::enqueue_email_notification(self.auth.inner(), to, subject, message).await;
    }

    pub async fn rebuild_fitness_freshness(&self, user_id: i32) {
        self.enqueue_task(Task::RebuildFitnessFreshness(RebuildFitnessFreshnessTask {
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

        self.enqueue_task(Task::RebuildSegmentAnalytics(RebuildSegmentAnalyticsTask {
            segment_ids,
        }))
        .await;
    }

    pub async fn enqueue(&self, task: Task) {
        self.enqueue_with_options(task, None, 3).await
    }

    pub async fn enqueue_with_options(
        &self,
        task: Task,
        scheduled_for: Option<chrono::DateTime<Utc>>,
        max_attempts: i32,
    ) {
        self.enqueue_task_with_options(task, scheduled_for, max_attempts)
            .await
    }
}

pub fn create_auth_service(db: DatabaseConnection, tasks: TaskQueue) -> AppAuthService {
    let metrics = kaleido::auth::FnMetricsRecorder::new(
        || kaleido::glass::api_metrics::login_counter().inc(),
        || kaleido::glass::api_metrics::failed_login_counter().inc(),
        || kaleido::glass::api_metrics::logout_counter().inc(),
        || kaleido::glass::api_metrics::token_refresh_counter().inc(),
    );
    kaleido::auth::build_default_auth_service(
        Arc::new(db),
        kaleido::auth::AuthEmailService::new(tasks.auth_queue().clone()),
        kaleido::auth::EnvConfigProvider::from_env(),
        metrics,
    )
}
