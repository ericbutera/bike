pub mod processors;

use api::config::Config;
use kaleido::auth::worker::{
    register_all_auth_processors as register_shared_auth_processors, AuthWorkerConfig,
    AuthWorkerSmtpConfig,
};
use kaleido::background_jobs::worker::{TaskWorker, WorkerError};
use std::sync::Arc;

pub use processors::*;

pub fn register_auth_email_processors(
    worker: TaskWorker,
    cfg: &Config,
) -> Result<TaskWorker, WorkerError> {
    let auth_worker_config = AuthWorkerConfig::new(
        cfg.app_name.clone(),
        AuthWorkerSmtpConfig {
            host: cfg.smtp_host.clone(),
            port: cfg.smtp_port,
            username: cfg.smtp_username.clone(),
            password: cfg.smtp_password.clone(),
            from_email: cfg.smtp_from_email.clone(),
            from_name: cfg.smtp_from_name.clone(),
        },
    );
    let worker = register_shared_auth_processors(worker, &auth_worker_config)?;
    let email_notification = Arc::new(EmailNotification::new(cfg)?);

    Ok(worker.register_processor(email_notification))
}

pub async fn register_default_processors(worker: TaskWorker) -> Result<TaskWorker, WorkerError> {
    Ok(worker)
}
