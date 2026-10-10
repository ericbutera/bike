use crate::background_jobs::storage::TaskStorage;
use crate::background_jobs::TaskQueue;
use serde::{Deserialize, Serialize};

pub const EMAIL_NOTIFICATION_TASK_TYPE: &str = "email_notification";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailNotificationTask {
    pub to: String,
    pub subject: String,
    pub message: String,
}

pub async fn enqueue_email_notification<S: TaskStorage>(
    queue: &TaskQueue<S>,
    to: String,
    subject: String,
    message: String,
) -> Result<(), crate::background_jobs::TaskError> {
    queue
        .enqueue(
            EMAIL_NOTIFICATION_TASK_TYPE.to_string(),
            EmailNotificationTask {
                to,
                subject,
                message,
            },
        )
        .await
        .map(|_| ())
}
