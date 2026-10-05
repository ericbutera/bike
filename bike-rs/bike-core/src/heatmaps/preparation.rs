use super::{
    geometry,
    projection::{PendingProjection, Projection},
};
use crate::activity_data::deserialize_derived_activity_data;
use crate::activity_sport::normalize_activity_sport;
use sea_orm::{DatabaseConnection, DbErr};

pub async fn prepare_activity(
    db: &DatabaseConnection,
    pending: PendingProjection,
) -> Result<(), DbErr> {
    let Some(source) = Projection::source(db, &pending).await? else {
        return Ok(());
    };
    let chunks = if should_exclude_from_heatmap(&source.sport, &source.source, &source.title) {
        Vec::new()
    } else {
        let derived = deserialize_derived_activity_data(source.derived_data_json.as_ref());
        tokio::task::spawn_blocking(move || geometry::prepare(&derived.route_points))
            .await
            .map_err(|error| DbErr::Custom(error.to_string()))?
    };
    Projection::publish(db, &pending, &chunks).await?;
    Ok(())
}

fn should_exclude_from_heatmap(sport: &str, source: &str, title: &str) -> bool {
    normalize_activity_sport(sport) == "indoor_trainer_ride"
        || sport.to_ascii_lowercase().contains("virtual")
        || source.to_ascii_lowercase().contains("zwift")
        || title.to_ascii_lowercase().contains("zwift")
}

#[cfg(test)]
mod tests {
    use super::should_exclude_from_heatmap;

    #[test]
    fn excludes_virtual_and_zwift_rides_but_keeps_outdoor_rides() {
        assert!(should_exclude_from_heatmap(
            "VirtualRide",
            "strava_sync",
            "Morning ride"
        ));
        assert!(should_exclude_from_heatmap(
            "road_ride",
            "strava_sync",
            "Zwift morning ride"
        ));
        assert!(should_exclude_from_heatmap(
            "indoor trainer ride",
            "manual_upload",
            "Morning ride"
        ));
        assert!(!should_exclude_from_heatmap(
            "road_ride",
            "strava_sync",
            "Morning ride"
        ));
    }
}
