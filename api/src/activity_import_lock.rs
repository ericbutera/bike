use crate::app_error::AppError;
use bike_core::activity_import_lock as core_activity_import_lock;
use bike_core::activity_import_lock::ActivityImportLockError;
use bike_core::entities::activity_import_locks;
use sea_orm::DatabaseConnection;

pub use bike_core::activity_import_lock::{
    describe_source, describe_stage, ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT, ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP,
    ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD, ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
    ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC, ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL,
    ACTIVITY_IMPORT_LOCK_STAGE_QUEUED, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};

impl From<ActivityImportLockError> for AppError {
    fn from(error: ActivityImportLockError) -> Self {
        match error {
            ActivityImportLockError::Conflict(message) => Self::conflict(message),
            ActivityImportLockError::Internal(message) => Self::internal(message),
            ActivityImportLockError::Database(error) => Self::from(error),
        }
    }
}

pub async fn acquire_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, AppError> {
    core_activity_import_lock::acquire_user_activity_import_lock(db, user_id, source, stage)
        .await
        .map_err(AppError::from)
}

pub async fn mark_user_activity_import_lock_stage(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, AppError> {
    core_activity_import_lock::mark_user_activity_import_lock_stage(db, user_id, source, stage)
        .await
        .map_err(AppError::from)
}

pub async fn ensure_user_activity_import_lock_stage(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, AppError> {
    core_activity_import_lock::ensure_user_activity_import_lock_stage(db, user_id, source, stage)
        .await
        .map_err(AppError::from)
}

pub async fn release_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
) -> Result<(), AppError> {
    core_activity_import_lock::release_user_activity_import_lock(db, user_id, source)
        .await
        .map_err(AppError::from)
}

pub async fn load_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Option<activity_import_locks::Model>, AppError> {
    core_activity_import_lock::load_user_activity_import_lock(db, user_id)
        .await
        .map_err(AppError::from)
}
