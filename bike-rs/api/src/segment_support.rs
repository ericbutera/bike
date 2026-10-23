use crate::app_error::AppError;
use bike_core::activity_data::{ActivityRoutePoint, StoredRoutePointSeries};
use sea_orm::{ConnectionTrait, TransactionTrait};

pub use bike_core::segment_support::{deserialize_segment_route_points, slice_effort_route_points};

pub fn serialize_segment_route_points(route_points: &[ActivityRoutePoint]) -> Result<StoredRoutePointSeries, AppError> {
    Ok(bike_core::segment_support::serialize_segment_route_points(route_points))
}

pub async fn clear_segment_efforts_for_activity<C>(db: &C, user_id: i32, activity_id: i32) -> Result<(), AppError>
where
    C: ConnectionTrait,
{
    bike_core::segment_support::clear_segment_efforts_for_activity(db, user_id, activity_id)
        .await
        .map_err(|error| AppError::internal(error.message))
}

pub async fn replace_segment_efforts_for_activity<C>(
    db: &C,
    user_id: i32,
    activity_id: i32,
    activity_route_points: &[ActivityRoutePoint],
) -> Result<(), AppError>
where
    C: ConnectionTrait + TransactionTrait,
{
    bike_core::segment_support::replace_segment_efforts_for_activity(db, user_id, activity_id, activity_route_points)
        .await
        .map_err(|error| AppError::internal(error.message))
}
