use crate::activity_details::ActivityRoutePoint;
use crate::activity_import_lock::{
    mark_user_activity_import_lock_stage, release_user_activity_import_lock,
    ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION, ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::activity_import_pipeline::{
    finalize_activity_import_batch, reprocess_activity_from_import,
};
use crate::analytics::mark_segment_activity_changes;
use crate::analytics::rebuild_segment_analytics_cache;
use crate::app_error::AppError;
use crate::entities::{activities, activity_imports, segment_efforts};
use crate::segment_support::{
    clear_segment_efforts_for_activity, replace_segment_efforts_for_activity,
};
use crate::tasks::TaskQueue;
use crate::training_profile::load_training_profile;
use chrono::Utc;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    TransactionTrait,
};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub async fn refresh_activity_derived_state<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    route_points: &[ActivityRoutePoint],
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    let mut affected_segment_ids = load_segment_ids_for_activity(db, activity_id).await?;

    replace_segment_efforts_for_activity(db, user_id, activity_id, route_points).await?;

    affected_segment_ids.extend(load_segment_ids_for_activity(db, activity_id).await?);
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    rebuild_segment_analytics_cache(db, &affected_segment_ids).await?;

    Ok(affected_segment_ids)
}

pub async fn delete_activity_with_derived_state(
    db: &DatabaseConnection,
    uploads_dir: &str,
    user_id: i32,
    activity: activities::Model,
) -> Result<Vec<i32>, AppError> {
    let mut upload_path_to_remove = None;
    let txn = db.begin().await?;
    let affected_segment_ids = load_segment_ids_for_activity(&txn, activity.id).await?;

    clear_segment_efforts_for_activity(&txn, user_id, activity.id).await?;

    activities::Entity::delete_many()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::Id.eq(activity.id))
        .exec(&txn)
        .await?;

    if let Some(activity_import_id) = activity.activity_import_id {
        let remaining_references = activities::Entity::find()
            .filter(activities::Column::UserId.eq(user_id))
            .filter(activities::Column::ActivityImportId.eq(Some(activity_import_id)))
            .count(&txn)
            .await?;

        if remaining_references == 0 {
            let linked_import = activity_imports::Entity::find()
                .filter(activity_imports::Column::Id.eq(activity_import_id))
                .filter(activity_imports::Column::UserId.eq(user_id))
                .one(&txn)
                .await?;

            if let Some(linked_import) = linked_import {
                upload_path_to_remove =
                    Some(Path::new(uploads_dir).join(&linked_import.storage_path));

                activity_imports::Entity::delete_many()
                    .filter(activity_imports::Column::Id.eq(activity_import_id))
                    .filter(activity_imports::Column::UserId.eq(user_id))
                    .exec(&txn)
                    .await?;
            }
        }
    }

    txn.commit().await?;

    if let Some(upload_path) = upload_path_to_remove {
        remove_upload_file(upload_path).await;
    }

    Ok(affected_segment_ids)
}

pub async fn load_segment_ids_for_activity<C>(
    db: &C,
    activity_id: i32,
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait,
{
    let mut segment_ids = segment_efforts::Entity::find()
        .filter(segment_efforts::Column::ActivityId.eq(activity_id))
        .all(db)
        .await?
        .into_iter()
        .map(|effort| effort.segment_id)
        .collect::<Vec<_>>();
    segment_ids.sort_unstable();
    segment_ids.dedup();

    Ok(segment_ids)
}

pub async fn regenerate_segments_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Vec<i32>, AppError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let mut affected_segment_ids = Vec::new();

    for activity in activities {
        let route_points = crate::activity_details::deserialize_derived_activity_data(
            activity.derived_data_json.as_deref(),
        )
        .route_points;

        affected_segment_ids
            .extend(refresh_activity_derived_state(db, user_id, activity.id, &route_points).await?);
    }

    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    if !affected_segment_ids.is_empty() {
        mark_segment_activity_changes(db, &affected_segment_ids, Utc::now()).await?;
    }

    Ok(affected_segment_ids)
}

pub async fn process_user_segment_regeneration(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), AppError> {
    mark_user_activity_import_lock_stage(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = regenerate_segments_for_user(db, user_id).await;
    let release_result = release_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(_), Ok(())) => Ok(()),
    }
}

pub async fn reprocess_imported_activities_for_user(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(usize, usize), AppError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .filter(activities::Column::ActivityImportId.is_not_null())
        .all(db)
        .await?;
    let import_ids = activities
        .iter()
        .filter_map(|activity| activity.activity_import_id)
        .collect::<Vec<_>>();

    if import_ids.is_empty() {
        return Ok((0, 0));
    }

    let imports_by_id = activity_imports::Entity::find()
        .filter(activity_imports::Column::UserId.eq(user_id))
        .filter(activity_imports::Column::Id.is_in(import_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|activity_import| (activity_import.id, activity_import))
        .collect::<HashMap<_, _>>();
    let training_profile = load_training_profile(db, user_id).await?;
    let mut reprocessed_count = 0usize;
    let mut failed_count = 0usize;
    let mut affected_segment_ids = Vec::new();

    for activity in activities {
        let Some(activity_import_id) = activity.activity_import_id else {
            continue;
        };
        let activity_id = activity.id;
        let Some(activity_import) = imports_by_id.get(&activity_import_id).cloned() else {
            failed_count += 1;
            tracing::warn!(
                user_id,
                activity_id,
                activity_import_id,
                "skipping activity reprocess because the linked import record is missing"
            );
            continue;
        };

        match reprocess_activity_from_import(
            db,
            uploads_dir,
            user_id,
            activity,
            activity_import,
            Some(&training_profile),
        )
        .await
        {
            Ok(reprocessed) => {
                reprocessed_count += 1;
                affected_segment_ids.extend(reprocessed.affected_segment_ids);
            }
            Err(error) => {
                failed_count += 1;
                tracing::warn!(
                    user_id,
                    activity_id,
                    error = %error.message,
                    "failed to reprocess imported activity from stored source file"
                );
            }
        }
    }

    if reprocessed_count > 0 {
        finalize_activity_import_batch(db, tasks, user_id, affected_segment_ids, Utc::now())
            .await?;
    }

    Ok((reprocessed_count, failed_count))
}

pub async fn process_user_activity_import_reprocessing(
    db: &DatabaseConnection,
    uploads_dir: &str,
    tasks: &TaskQueue,
    user_id: i32,
) -> Result<(), AppError> {
    mark_user_activity_import_lock_stage(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
        ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
    )
    .await?;

    let result = reprocess_imported_activities_for_user(db, uploads_dir, tasks, user_id).await;
    let release_result = release_user_activity_import_lock(
        db,
        user_id,
        ACTIVITY_IMPORT_LOCK_SOURCE_ACTIVITY_REPROCESSING,
    )
    .await;

    match (result, release_result) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok((reprocessed_count, failed_count)), Ok(())) => {
            tracing::info!(
                user_id,
                reprocessed_count,
                failed_count,
                "completed user activity import reprocessing"
            );
            Ok(())
        }
    }
}

async fn remove_upload_file(path: PathBuf) {
    if let Err(error) = tokio::fs::remove_file(&path).await {
        if error.kind() != ErrorKind::NotFound {
            tracing::warn!(
                error = ?error,
                path = %path.display(),
                "failed to remove upload file for deleted activity"
            );
        }
    }
}
