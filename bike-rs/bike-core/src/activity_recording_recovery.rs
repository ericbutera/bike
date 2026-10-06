//! Recover metadata stripped by legacy exports without deleting provider identities.
use crate::activity_data::{
    deserialize_derived_activity_data, ActivityDerivedData, ActivityRoutePoint,
};
use crate::activity_recording::{RecordingContext, RecordingEvidence};
use crate::entities::activities;
use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbErr};

pub async fn propagate_recording(
    db: &impl ConnectionTrait,
    activity: &activities::Model,
) -> Result<(), DbErr> {
    let context = activity.recording_context();
    if !context.excluded() {
        return Ok(());
    }
    let original = deserialize_derived_activity_data(activity.derived_data_json.as_ref());
    for candidate in activities::Model::recording_counterparts(
        db,
        activity.user_id,
        activity.id,
        activity.started_at,
    )
    .await?
    {
        let mut derived = deserialize_derived_activity_data(candidate.derived_data_json.as_ref());
        let legacy =
            RecordingContext::legacy(&candidate.sport, &candidate.source, &candidate.title);
        if derived.recording.excluded() || legacy.excluded() {
            continue;
        }
        if routes_match(
            candidate.started_at,
            &derived.route_points,
            activity.started_at,
            &original.route_points,
        ) {
            derived.recording.merge(context.clone());
            derived.recording.evidence.push(RecordingEvidence {
                field: "verified_activity_counterpart".into(),
                value: activity.id.to_string(),
            });
            activities::Model::store_recording(
                db,
                candidate.id,
                activity.user_id,
                candidate.updated_at,
                &derived,
            )
            .await?;
        }
    }
    Ok(())
}

pub async fn resolve_recording(
    db: &impl ConnectionTrait,
    activity_id: i32,
    user_id: i32,
    started_at: DateTime<Utc>,
    derived: &ActivityDerivedData,
) -> Result<RecordingContext, DbErr> {
    let mut context = derived.recording.clone();
    if context.excluded() || derived.route_points.len() < 8 {
        return Ok(context);
    }
    for candidate in
        activities::Model::recording_counterparts(db, user_id, activity_id, started_at).await?
    {
        let original = deserialize_derived_activity_data(candidate.derived_data_json.as_ref());
        let mut evidence = original.recording;
        evidence.merge(RecordingContext::legacy(
            &candidate.sport,
            &candidate.source,
            &candidate.title,
        ));
        if evidence.excluded()
            && routes_match(
                started_at,
                &derived.route_points,
                candidate.started_at,
                &original.route_points,
            )
        {
            evidence.evidence.push(RecordingEvidence {
                field: "verified_activity_counterpart".into(),
                value: candidate.id.to_string(),
            });
            context.merge(evidence);
        }
    }
    Ok(context)
}

pub fn routes_match(
    start: DateTime<Utc>,
    route: &[ActivityRoutePoint],
    original_start: DateTime<Utc>,
    original: &[ActivityRoutePoint],
) -> bool {
    if route.len() < 8
        || original.len() < 8
        || route
            .windows(2)
            .any(|p| p[0].elapsed_seconds > p[1].elapsed_seconds)
        || original
            .windows(2)
            .any(|p| p[0].elapsed_seconds > p[1].elapsed_seconds)
    {
        return false;
    }
    if !route.iter().any(|p| {
        crate::dedupe::haversine_distance_meters(
            route[0].latitude,
            route[0].longitude,
            p.latitude,
            p.longitude,
        ) > 100.0
    }) {
        return false;
    }
    let samples = route.len().min(64);
    let matches = (0..samples)
        .filter(|index| {
            let point = &route[index * (route.len() - 1) / (samples - 1)];
            let elapsed =
                start.timestamp() + i64::from(point.elapsed_seconds) - original_start.timestamp();
            let Ok(elapsed) = i32::try_from(elapsed) else {
                return false;
            };
            original
                .binary_search_by_key(&elapsed, |p| p.elapsed_seconds)
                .is_ok_and(|index| {
                    let other = &original[index];
                    crate::dedupe::haversine_distance_meters(
                        point.latitude,
                        point.longitude,
                        other.latitude,
                        other.longitude,
                    ) <= 2.0
                })
        })
        .count();
    matches * 100 >= samples * 95
}

#[cfg(test)]
mod tests {
    use super::*;

    fn route() -> Vec<ActivityRoutePoint> {
        (0..16)
            .map(|i| ActivityRoutePoint {
                elapsed_seconds: i * 5,
                latitude: 45.0,
                longitude: -120.0 + f64::from(i) * 0.0002,
                distance_meters: None,
                elevation_meters: None,
                speed_mps: None,
                heart_rate_bpm: None,
                cadence_rpm: None,
                power_watts: None,
            })
            .collect()
    }

    #[test]
    fn stripped_copy_matches_using_absolute_gps_time_despite_shifted_start() {
        let start = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let original = route();
        let mut copy = original.clone();
        for point in &mut copy {
            point.elapsed_seconds -= 2;
            point.latitude += 0.0000001;
        }
        assert!(routes_match(
            start + chrono::Duration::seconds(2),
            &copy,
            start,
            &original
        ));
    }

    #[test]
    fn nearby_or_short_or_differently_timed_real_rides_are_not_counterparts() {
        let start = DateTime::from_timestamp(1_600_000_000, 0).unwrap();
        let original = route();
        assert!(!routes_match(
            start + chrono::Duration::seconds(1),
            &original,
            start,
            &original
        ));
        let mut nearby = original.clone();
        for point in &mut nearby {
            point.latitude += 0.0001;
        }
        assert!(!routes_match(start, &nearby, start, &original));
        assert!(!routes_match(start, &original[..7], start, &original));
        let mut reversed = original.clone();
        reversed.reverse();
        assert!(!routes_match(start, &reversed, start, &original));
    }
}
