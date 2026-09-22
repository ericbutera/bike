use crate::activity_data::deserialize_derived_activity_data;
use crate::activity_import_lock::{
    mark_user_activity_import_lock_stage, release_user_activity_import_lock,
    ActivityImportLockError, ACTIVITY_IMPORT_LOCK_SOURCE_SEGMENT_REGENERATION,
    ACTIVITY_IMPORT_LOCK_STAGE_RUNNING,
};
use crate::analytics::{
    mark_segment_activity_changes, rebuild_activity_analytics_cache,
    rebuild_segment_analytics_cache,
};
use crate::entities::{activities, segment_efforts, segments};
use crate::segment_support::{
    deserialize_segment_route_points, replace_segment_efforts_for_activity,
    replace_segment_efforts_for_segment, SegmentSupportError,
};
use chrono::Utc;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use std::fmt;

#[derive(Debug)]
pub struct SegmentRegenerationError {
    pub message: String,
}

impl SegmentRegenerationError {
    fn internal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for SegmentRegenerationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for SegmentRegenerationError {}

impl From<DbErr> for SegmentRegenerationError {
    fn from(error: DbErr) -> Self {
        Self::internal(error.to_string())
    }
}

impl From<ActivityImportLockError> for SegmentRegenerationError {
    fn from(error: ActivityImportLockError) -> Self {
        Self::internal(error.to_string())
    }
}

impl From<SegmentSupportError> for SegmentRegenerationError {
    fn from(error: SegmentSupportError) -> Self {
        Self::internal(error.message)
    }
}

pub async fn regenerate_segment_efforts(
    db: &DatabaseConnection,
    segment_id: i32,
) -> Result<(), SegmentRegenerationError> {
    let segment = segments::Entity::find_by_id(segment_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            SegmentRegenerationError::internal(format!("segment {segment_id} was not found"))
        })?;
    let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());

    let affected_activity_ids =
        replace_segment_efforts_for_segment(db, segment.user_id, segment.id, &route_points).await?;
    mark_segment_activity_changes(db, &[segment.id], Utc::now()).await?;
    rebuild_segment_analytics_cache(db, &[segment.id]).await?;
    rebuild_activity_analytics_cache(db, &affected_activity_ids).await?;

    Ok(())
}

pub async fn regenerate_segments_for_user(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<Vec<i32>, SegmentRegenerationError> {
    let activities = activities::Entity::find()
        .filter(activities::Column::UserId.eq(user_id))
        .all(db)
        .await?;
    let mut affected_segment_ids = Vec::new();

    for activity in activities {
        let route_points =
            deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;

        let affected_for_activity =
            refresh_activity_segment_efforts(db, user_id, activity.id, &route_points).await?;
        affected_segment_ids.extend(affected_for_activity);
    }

    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    if !affected_segment_ids.is_empty() {
        mark_segment_activity_changes(db, &affected_segment_ids, Utc::now()).await?;
    }

    Ok(affected_segment_ids)
}

async fn refresh_activity_segment_efforts(
    db: &DatabaseConnection,
    user_id: i32,
    activity_id: i32,
    route_points: &[crate::activity_data::ActivityRoutePoint],
) -> Result<Vec<i32>, SegmentRegenerationError> {
    let mut affected_segment_ids = load_segment_ids_for_activity(db, activity_id).await?;

    replace_segment_efforts_for_activity(db, user_id, activity_id, route_points).await?;

    affected_segment_ids.extend(load_segment_ids_for_activity(db, activity_id).await?);
    affected_segment_ids.sort_unstable();
    affected_segment_ids.dedup();

    rebuild_segment_analytics_cache(db, &affected_segment_ids).await?;
    rebuild_activity_analytics_cache(db, &[activity_id]).await?;

    Ok(affected_segment_ids)
}

async fn load_segment_ids_for_activity(
    db: &DatabaseConnection,
    activity_id: i32,
) -> Result<Vec<i32>, DbErr> {
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

pub async fn process_user_segment_regeneration(
    db: &DatabaseConnection,
    user_id: i32,
) -> Result<(), SegmentRegenerationError> {
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
        (Ok(_), Err(error)) => Err(error.into()),
        (Ok(_), Ok(())) => Ok(()),
    }
}
