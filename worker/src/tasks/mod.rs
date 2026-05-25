pub mod processors;

use api::config::Config;
use kaleido::auth::worker::{
    register_all_auth_processors as register_shared_auth_processors, AuthWorkerConfig,
    AuthWorkerSmtpConfig,
};
use kaleido::background_jobs::worker::{TaskWorker, WorkerError};
use sea_orm::DatabaseConnection;
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

pub async fn register_default_processors(
    worker: TaskWorker,
    db: DatabaseConnection,
) -> Result<TaskWorker, WorkerError> {
    let backfill_user_xc_training = Arc::new(BackfillUserXcTraining::new(db.clone()));
    let rebuild_fitness_freshness = Arc::new(RebuildFitnessFreshness::new(db.clone()));
    let rebuild_segment_analytics = Arc::new(RebuildSegmentAnalytics::new(db.clone()));
    let reprocess_user_activity_imports = Arc::new(ReprocessUserActivityImports::new(db.clone()));
    let regenerate_user_segments = Arc::new(RegenerateUserSegments::new(db.clone()));
    let activity_archive_import = Arc::new(ActivityArchiveImport::new(db.clone()));
    let strava_sync = Arc::new(StravaSync::new(db));

    Ok(worker
        .register_processor(backfill_user_xc_training)
        .register_processor(rebuild_fitness_freshness)
        .register_processor(rebuild_segment_analytics)
        .register_processor(reprocess_user_activity_imports)
        .register_processor(regenerate_user_segments)
        .register_processor(activity_archive_import)
        .register_processor(strava_sync))
}
