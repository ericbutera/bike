use super::*;
use crate::activity_details::ActivityRoutePoint;
use crate::dedupe::segment_dedupe_key;
use crate::segment_support::deserialize_segment_route_points;
const SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS: f64 = 5.0;
#[derive(Clone, Debug, sea_orm::FromQueryResult)]
pub struct SegmentListRow {
    pub id: i32,
    pub title: String,
    pub source: String,
    pub mode: String,
    pub starred: bool,
    pub original_filename: Option<String>,
    pub format: Option<String>,
    pub distance_meters: Option<f64>,
    pub source_activity_id: Option<i32>,
    pub source_start_route_point_index: Option<i32>,
    pub source_end_route_point_index: Option<i32>,
    pub last_activity_change_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}
#[derive(Clone, Debug, sea_orm::FromQueryResult)]
pub struct SegmentDedupeCandidateRow {
    pub id: i32,
    pub distance_meters: Option<f64>,
    pub route_data_json: Option<crate::activity_details::StoredRoutePointSeries>,
}
impl Model {
    pub async fn find_duplicate(
        db: &impl ConnectionTrait,
        user_id: i32,
        distance_meters: Option<f64>,
        route_points: &[ActivityRoutePoint],
    ) -> Result<Option<Model>, DbErr> {
        let Some(target_key) = segment_dedupe_key(distance_meters, route_points) else {
            return Ok(None);
        };

        let mut candidate_query = Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::DistanceMeters)
            .column(Column::RouteDataJson)
            .filter(Column::UserId.eq(user_id));

        if let Some((minimum_distance, maximum_distance)) =
            segment_dedupe_distance_bucket(distance_meters)
        {
            candidate_query = candidate_query
                .filter(Column::DistanceMeters.gte(minimum_distance))
                .filter(Column::DistanceMeters.lt(maximum_distance));
        } else {
            candidate_query = candidate_query.filter(Column::DistanceMeters.is_null());
        }

        let candidates = candidate_query
            .into_model::<SegmentDedupeCandidateRow>()
            .all(db)
            .await?;

        for segment in candidates {
            let candidate_route_points =
                deserialize_segment_route_points(segment.route_data_json.as_ref());
            if segment_dedupe_key(segment.distance_meters, &candidate_route_points).as_deref()
                == Some(target_key.as_str())
            {
                return Self::find_owned(db, segment.id, user_id).await;
            }
        }

        Ok(None)
    }
    pub async fn list_for_user(
        db: &impl ConnectionTrait,
        user_id: i32,
    ) -> Result<Vec<SegmentListRow>, DbErr> {
        Entity::find()
            .select_only()
            .column(Column::Id)
            .column(Column::Title)
            .column(Column::Source)
            .column(Column::Mode)
            .column(Column::Starred)
            .column(Column::OriginalFilename)
            .column(Column::Format)
            .column(Column::DistanceMeters)
            .column(Column::SourceActivityId)
            .column(Column::SourceStartRoutePointIndex)
            .column(Column::SourceEndRoutePointIndex)
            .column(Column::LastActivityChangeAt)
            .column(Column::CreatedAt)
            .filter(Column::UserId.eq(user_id))
            .into_model::<SegmentListRow>()
            .all(db)
            .await
    }
}
fn segment_dedupe_distance_bucket(distance_meters: Option<f64>) -> Option<(f64, f64)> {
    let distance_meters = distance_meters.filter(|value| value.is_finite() && *value > 0.0)?;
    let minimum = (distance_meters / SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS).floor()
        * SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS;

    Some((minimum, minimum + SEGMENT_DEDUPE_DISTANCE_BUCKET_METERS))
}
