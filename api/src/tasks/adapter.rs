use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub use bike_core::jobs::{
    ActivityArchiveImportTask, BackfillUserXcTrainingTask, Job as Task, JobQueue as TaskQueue,
    ProcessActivityImportTask, QueuedJobReference as QueuedTaskReference,
    RebuildFitnessFreshnessTask, RebuildSegmentAnalyticsTask, RegenerateSegmentEffortsTask,
    RegenerateUserSegmentsTask, ReprocessActivityImportTask, ReprocessUserActivityImportsTask,
    StravaSyncTask,
};

pub use kaleido::auth::DefaultEnvAuthService as AppAuthService;

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
