use crate::activity_data::deserialize_derived_activity_data;
use crate::analytics::{
    mark_segment_activity_changes, rebuild_activity_analytics_cache, rebuild_segment_analytics_cache,
};
use crate::entities::{activities, segments};
use crate::segment_support::{
    deserialize_segment_route_points, replace_segment_efforts_for_segment, SegmentSupportError,
};
use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr};
use std::fmt;

#[cfg(test)]
mod tests;

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

impl From<SegmentSupportError> for SegmentRegenerationError {
    fn from(error: SegmentSupportError) -> Self {
        Self::internal(error.message)
    }
}

pub async fn regenerate_segment_efforts(
    db: &DatabaseConnection,
    segment_id: i32,
) -> Result<(), SegmentRegenerationError> {
    let segment = segments::Model::find_by_id(db, segment_id)
        .await?
        .ok_or_else(|| SegmentRegenerationError::internal(format!("segment {segment_id} was not found")))?;
    let route_points = deserialize_segment_route_points(segment.route_data_json.as_ref());

    let affected_activity_ids =
        replace_segment_efforts_for_segment(db, segment.user_id, segment.id, &route_points).await?;
    mark_segment_activity_changes(db, &[segment.id], Utc::now()).await?;
    rebuild_segment_analytics_cache(db, &[segment.id]).await?;
    rebuild_activity_analytics_cache(db, &affected_activity_ids).await?;

    Ok(())
}

pub async fn match_activity(
    db: &DatabaseConnection,
    activity: &activities::Model,
) -> Result<Vec<i32>, crate::workflow_error::WorkflowError> {
    use sea_orm::TransactionTrait;
    let txn = db.begin().await?;
    let current = activities::Model::lock_owned(&txn, activity.id, activity.user_id)
        .await?
        .ok_or_else(|| crate::workflow_error::WorkflowError::not_found("Activity was deleted"))?;
    let route = if current.is_bike_activity() {
        deserialize_derived_activity_data(current.derived_data_json.as_ref()).route_points
    } else {
        Vec::new()
    };
    let mut ids = crate::entities::segment_user_summaries::Model::segment_ids_for_user(&txn, current.user_id).await?;
    ids.extend(
        crate::activity_lifecycle::refresh_activity_derived_state_without_cache_rebuilds(
            &txn,
            current.user_id,
            current.id,
            &route,
        )
        .await?,
    );
    ids.sort_unstable();
    ids.dedup();
    segments::Model::mark_activity_changes(&txn, &ids, Utc::now()).await?;
    txn.commit().await?;
    Ok(ids)
}
