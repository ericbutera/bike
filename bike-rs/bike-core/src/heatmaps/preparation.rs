use super::{
    geometry,
    projection::{PendingProjection, Projection},
};
use crate::activity_data::deserialize_derived_activity_data;
use sea_orm::{DatabaseConnection, DbErr};

pub async fn prepare_activity(
    db: &DatabaseConnection,
    pending: PendingProjection,
) -> Result<(), DbErr> {
    let Some(source) = Projection::source(db, &pending).await? else {
        return Ok(());
    };
    if !crate::activity_sport::is_bike_activity_sport(&source.sport)
        || source.format.as_deref() == Some("unavailable")
    {
        Projection::publish(db, &pending, &[]).await?;
        return Ok(());
    }
    let mut derived = deserialize_derived_activity_data(source.derived_data_json.as_ref());
    derived
        .recording
        .merge(crate::activity_recording::RecordingContext::legacy(
            &source.sport,
            &source.source,
            &source.title,
        ));
    let recovered = crate::activity_recording_recovery::resolve_recording(
        db,
        pending.activity_id,
        source.user_id,
        source.started_at,
        &derived,
    )
    .await?;
    if recovered != derived.recording {
        derived.recording = recovered;
        // Persist evidence with a source-version guard. The activity trigger queues a
        // fresh generation, so the original lease must not publish stale geometry.
        crate::entities::activities::Model::store_recording(
            db,
            pending.activity_id,
            source.user_id,
            source.updated_at,
            &derived,
        )
        .await?;
        return Ok(());
    }
    let chunks = tokio::task::spawn_blocking(move || prepare_route(&derived))
        .await
        .map_err(|error| DbErr::Custom(error.to_string()))?;
    Projection::publish(db, &pending, &chunks).await?;
    Ok(())
}

pub(crate) fn prepare_route(
    derived: &crate::activity_data::ActivityDerivedData,
) -> Vec<geometry::Chunk> {
    if derived.recording.excluded() {
        Vec::new()
    } else {
        geometry::prepare(&derived.route_points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        activity_details::derive_activity_detail_data, activity_recording::RecordingContext,
    };

    #[test]
    fn legacy_virtual_and_zwift_flags_remove_chunks_while_real_rides_keep_them() {
        for (sport, source, title, excluded) in [
            ("VirtualRide", "strava_sync", "Morning ride", true),
            ("road_ride", "strava_sync", "Zwift morning ride", true),
            ("indoor trainer ride", "manual_upload", "Morning ride", true),
            ("road_ride", "strava_sync", "Morning ride", false),
        ] {
            let mut derived = derive_activity_detail_data(
                "outdoor.fit",
                "fit",
                include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
            )
            .unwrap();
            derived
                .recording
                .merge(RecordingContext::legacy(sport, source, title));
            assert_eq!(prepare_route(&derived).is_empty(), excluded);
        }
    }
}

#[cfg(test)]
mod recording_tests {
    use super::*;
    use crate::activity_details::derive_activity_detail_data;

    #[test]
    fn real_routes_in_multiple_countries_contribute_without_geography_classification() {
        let original = derive_activity_detail_data(
            "outdoor.fit",
            "fit",
            include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
        )
        .unwrap();
        for (latitude, longitude) in [(45.0, -120.0), (52.0, 0.0), (35.0, 140.0)] {
            let mut derived = original.clone();
            for (index, point) in derived.route_points.iter_mut().enumerate() {
                point.latitude = latitude;
                point.longitude = longitude + index as f64 * 0.0002;
            }
            assert!(!prepare_route(&derived).is_empty());
            derived
                .recording
                .observe("fit.creator.manufacturer", "zwift");
            assert!(prepare_route(&derived).is_empty());
        }
    }
}
