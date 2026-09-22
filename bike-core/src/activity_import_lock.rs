use crate::entities::activity_import_locks;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use std::error::Error;
use std::fmt;

pub const ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT: &str = "archive_import";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING: &str = "activity_reprocessing";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP: &str = "duplicate_cleanup";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD: &str = "manual_upload";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION: &str = "segment_regeneration";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC: &str = "strava_sync";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL: &str = "xc_training_backfill";

pub const ACTIVITY_IMPORT_LOCK_STAGE_QUEUED: &str = "queued";
pub const ACTIVITY_IMPORT_LOCK_STAGE_RUNNING: &str = "running";

#[derive(Debug)]
pub enum ActivityImportLockError {
    Conflict(String),
    Internal(String),
    Database(DbErr),
}

impl ActivityImportLockError {
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::Conflict(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl fmt::Display for ActivityImportLockError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict(message) | Self::Internal(message) => formatter.write_str(message),
            Self::Database(error) => write!(formatter, "database request failed: {error}"),
        }
    }
}

impl Error for ActivityImportLockError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            Self::Conflict(_) | Self::Internal(_) => None,
        }
    }
}

impl From<DbErr> for ActivityImportLockError {
    fn from(error: DbErr) -> Self {
        Self::Database(error)
    }
}

pub async fn acquire_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, ActivityImportLockError> {
    let lock = activity_import_locks::ActiveModel {
        user_id: Set(user_id),
        source: Set(source.to_string()),
        stage: Set(stage.to_string()),
        ..Default::default()
    }
    .insert(db)
    .await;

    match lock {
        Ok(lock) => Ok(lock),
        Err(error) => {
            if let Some(existing) = load_user_activity_import_lock(db, user_id).await? {
                return Err(ActivityImportLockError::conflict(format!(
                    "Another activity processing operation is already {} for this user ({})",
                    describe_stage(&existing.stage),
                    describe_source(&existing.source),
                )));
            }

            Err(ActivityImportLockError::from(error))
        }
    }
}

pub async fn mark_user_activity_import_lock_stage(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, ActivityImportLockError> {
    let lock = load_user_activity_import_lock(db, user_id)
        .await?
        .ok_or_else(|| {
            ActivityImportLockError::internal(format!(
                "Activity import lock for user {user_id} was missing while starting {}",
                describe_source(source),
            ))
        })?;

    if lock.source != source {
        return Err(ActivityImportLockError::internal(format!(
            "Activity import lock for user {} is owned by {} instead of {}",
            user_id,
            describe_source(&lock.source),
            describe_source(source),
        )));
    }

    let mut active_model: activity_import_locks::ActiveModel = lock.into();
    active_model.stage = Set(stage.to_string());
    active_model
        .update(db)
        .await
        .map_err(ActivityImportLockError::from)
}

pub async fn ensure_user_activity_import_lock_stage(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, ActivityImportLockError> {
    let Some(lock) = load_user_activity_import_lock(db, user_id).await? else {
        tracing::warn!(
            user_id,
            source,
            stage,
            "activity import lock was missing; reacquiring lock for queued work"
        );
        return acquire_user_activity_import_lock(db, user_id, source, stage).await;
    };

    if lock.source != source {
        return Err(ActivityImportLockError::internal(format!(
            "Activity import lock for user {} is owned by {} instead of {}",
            user_id,
            describe_source(&lock.source),
            describe_source(source),
        )));
    }

    let mut active_model: activity_import_locks::ActiveModel = lock.into();
    active_model.stage = Set(stage.to_string());
    active_model
        .update(db)
        .await
        .map_err(ActivityImportLockError::from)
}

pub async fn release_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
) -> Result<(), ActivityImportLockError> {
    let Some(lock) = load_user_activity_import_lock(db, user_id).await? else {
        return Ok(());
    };

    if lock.source != source {
        tracing::warn!(
            user_id,
            expected_source = %source,
            actual_source = %lock.source,
            "skipping activity import lock release because another operation owns the lock"
        );
        return Ok(());
    }

    activity_import_locks::Entity::delete_by_id(lock.id)
        .exec(db)
        .await?;

    Ok(())
}

pub async fn load_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Option<activity_import_locks::Model>, ActivityImportLockError> {
    activity_import_locks::Entity::find()
        .filter(activity_import_locks::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(ActivityImportLockError::from)
}

pub fn describe_source(source: &str) -> &'static str {
    match source {
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT => "archive import",
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING => "activity reprocessing",
        ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP => "duplicate cleanup",
        ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD => "manual upload",
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION => "segment regeneration",
        ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC => "Strava sync",
        ACTIVITY_IMPORT_LOCK_SOURCE_XC_TRAINING_BACKFILL => "XC training backfill",
        _ => "activity processing",
    }
}

pub fn describe_stage(stage: &str) -> &'static str {
    match stage {
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED => "queued",
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING => "running",
        _ => "active",
    }
}
