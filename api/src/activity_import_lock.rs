use crate::app_error::AppError;
use crate::entities::activity_import_locks;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

pub const ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT: &str = "archive_import";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING: &str = "activity_reprocessing";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP: &str = "duplicate_cleanup";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD: &str = "manual_upload";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION: &str = "segment_regeneration";
pub const ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC: &str = "strava_sync";

pub const ACTIVITY_IMPORT_LOCK_STAGE_QUEUED: &str = "queued";
pub const ACTIVITY_IMPORT_LOCK_STAGE_RUNNING: &str = "running";

pub async fn acquire_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, AppError> {
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
                return Err(AppError::conflict(format!(
                    "Another activity processing operation is already {} for this user ({})",
                    describe_stage(&existing.stage),
                    describe_source(&existing.source),
                )));
            }

            Err(AppError::from(error))
        }
    }
}

pub async fn mark_user_activity_import_lock_stage(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, AppError> {
    let lock = load_user_activity_import_lock(db, user_id)
        .await?
        .ok_or_else(|| {
            AppError::internal(format!(
                "Activity import lock for user {} was missing while starting {}",
                user_id,
                describe_source(source),
            ))
        })?;

    if lock.source != source {
        return Err(AppError::internal(format!(
            "Activity import lock for user {} is owned by {} instead of {}",
            user_id,
            describe_source(&lock.source),
            describe_source(source),
        )));
    }

    let mut active_model: activity_import_locks::ActiveModel = lock.into();
    active_model.stage = Set(stage.to_string());
    active_model.update(db).await.map_err(AppError::from)
}

pub async fn ensure_user_activity_import_lock_stage(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
    stage: &str,
) -> Result<activity_import_locks::Model, AppError> {
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
        return Err(AppError::internal(format!(
            "Activity import lock for user {} is owned by {} instead of {}",
            user_id,
            describe_source(&lock.source),
            describe_source(source),
        )));
    }

    let mut active_model: activity_import_locks::ActiveModel = lock.into();
    active_model.stage = Set(stage.to_string());
    active_model.update(db).await.map_err(AppError::from)
}

pub async fn release_user_activity_import_lock(
    db: &DatabaseConnection,
    user_id: i32,
    source: &str,
) -> Result<(), AppError> {
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
) -> Result<Option<activity_import_locks::Model>, AppError> {
    activity_import_locks::Entity::find()
        .filter(activity_import_locks::Column::UserId.eq(user_id))
        .one(db)
        .await
        .map_err(AppError::from)
}

pub(crate) fn describe_source(source: &str) -> &'static str {
    match source {
        ACTIVITY_IMPORT_LOCK_SOURCE_ARCHIVE_IMPORT => "archive import",
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING => "activity reprocessing",
        ACTIVITY_IMPORT_LOCK_SOURCE_DUPLICATE_CLEANUP => "duplicate cleanup",
        ACTIVITY_IMPORT_LOCK_SOURCE_MANUAL_UPLOAD => "manual upload",
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION => "segment regeneration",
        ACTIVITY_IMPORT_LOCK_SOURCE_STRAVA_SYNC => "Strava sync",
        _ => "activity processing",
    }
}

pub(crate) fn describe_stage(stage: &str) -> &'static str {
    match stage {
        ACTIVITY_IMPORT_LOCK_STAGE_QUEUED => "queued",
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING => "running",
        _ => "active",
    }
}
