pub mod processors;
mod startup;

use bike_core::background_jobs::worker::{TaskWorker, WorkerError};
use bike_core::config::Config;
use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub use processors::*;
use startup::RecoverManualActivityImportsOnStartup;

pub fn register_email_processors(
    worker: TaskWorker,
    cfg: &Config,
) -> Result<TaskWorker, WorkerError> {
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
    let regenerate_segment_efforts = Arc::new(RegenerateSegmentEfforts::new(db.clone()));
    let process_activity_import = Arc::new(ProcessActivityImport::new(db.clone()));
    let reprocess_activity_import = Arc::new(ReprocessActivityImport::new(db.clone()));
    let reprocess_user_activity_imports = Arc::new(ReprocessUserActivityImports::new(db.clone()));
    let reprocess_archive_fit_activity_imports =
        Arc::new(ReprocessUserActivityImports::archive_fits_only(db.clone()));
    let regenerate_user_segments = Arc::new(RegenerateUserSegments::new(db.clone()));
    let activity_archive_import = Arc::new(ActivityArchiveImport::new(db.clone()));
    let prepare_heatmap = Arc::new(PrepareHeatmap::new(db.clone()));
    let strava_sync = Arc::new(StravaSync::new(db));
    let recover_manual_imports = Arc::new(RecoverManualActivityImportsOnStartup);

    Ok(worker
        .register_startup_hook(recover_manual_imports)
        .register_processor(backfill_user_xc_training)
        .register_processor(rebuild_fitness_freshness)
        .register_processor(rebuild_segment_analytics)
        .register_processor(regenerate_segment_efforts)
        .register_processor(process_activity_import)
        .register_processor(reprocess_activity_import)
        .register_processor(reprocess_user_activity_imports)
        .register_processor(reprocess_archive_fit_activity_imports)
        .register_processor(regenerate_user_segments)
        .register_processor(activity_archive_import)
        .register_processor(strava_sync)
        .register_processor(prepare_heatmap))
}
