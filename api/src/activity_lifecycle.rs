use crate::activity_details::ActivityRoutePoint;
use crate::app_error::AppError;
use crate::entities::{activities, activity_imports, segment_efforts};
use crate::segment_support::{
    clear_segment_efforts_for_activity, replace_segment_efforts_for_activity,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter,
    TransactionTrait,
};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

pub async fn refresh_activity_derived_state<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    route_points: &[ActivityRoutePoint],
) -> Result<Vec<i32>, AppError>
where
    C: ConnectionTrait,
{
    replace_segment_efforts_for_activity(db, user_id, activity_id, route_points).await?;
    load_segment_ids_for_activity(db, activity_id).await
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
