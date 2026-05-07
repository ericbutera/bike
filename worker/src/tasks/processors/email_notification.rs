use crate::email::EmailTemplate;
use api::config::Config;
use async_trait::async_trait;
use kaleido::auth::worker::tasks::EmailNotificationTask;
use kaleido::background_jobs::worker::TaskProcessor;
use kaleido::glass::email::{EmailService, SmtpConfig};
use serde_json::json;
use std::error::Error;
use std::sync::Arc;

pub struct EmailNotification {
    email_service: Arc<EmailService>,
    templates: EmailTemplate,
    config: Config,
}

impl EmailNotification {
    pub fn new(config: &Config) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let email_service = Arc::new(EmailService::new(&SmtpConfig {
            host: config.smtp_host.clone(),
            port: config.smtp_port,
            username: config.smtp_username.clone(),
            password: config.smtp_password.clone(),
            from_email: config.smtp_from_email.clone(),
            from_name: config.smtp_from_name.clone(),
        })?);

        Ok(Self {
            email_service,
            templates: EmailTemplate::new()?,
            config: config.clone(),
        })
    }
}

#[async_trait]
impl TaskProcessor for EmailNotification {
    fn task_type(&self) -> &str {
        "email_notification"
    }

    async fn process(
        &self,
        task_id: i32,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let data = payload.get("data").unwrap_or(&payload);
        let task: EmailNotificationTask = serde_json::from_value(data.clone())?;

        let template_data = json!({
            "app_name": self.config.app_name,
            "subject": task.subject,
            "message": task.message,
        });

        let text_body = self.templates.render("notification_text", &template_data)?;
        let html_body = self.templates.render("notification_html", &template_data)?;

        self.email_service
            .send(
                &task.to,
                &task.subject,
                text_body,
                html_body,
                Some(format!("notification/{}", task_id)),
            )
            .await
    }
}
