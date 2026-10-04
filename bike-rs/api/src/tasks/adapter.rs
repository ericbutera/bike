use sea_orm::DatabaseConnection;

pub use bike_core::jobs::{
    ActivityArchiveImportTask, BackfillUserXcTrainingTask, Job as Task, JobQueue as TaskQueue,
    ProcessActivityImportTask, QueuedJobReference as QueuedTaskReference,
    RebuildFitnessFreshnessTask, RebuildSegmentAnalyticsTask, RegenerateSegmentEffortsTask,
    RegenerateUserSegmentsTask, ReprocessActivityImportTask, ReprocessUserActivityImportsTask,
    StravaSyncTask,
};

pub use bike_core::auth::SessionService as AppSessionService;
pub fn create_session_service(db: DatabaseConnection) -> AppSessionService {
    AppSessionService::new(db, bike_core::config::Config::get().jwt_secret.clone())
}
