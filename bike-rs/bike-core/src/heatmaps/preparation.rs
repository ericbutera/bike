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
    let sport = normalize_activity_sport(&source.sport);
    let virtual_activity = sport == "indoor_trainer_ride"
        || source.sport.to_ascii_lowercase().contains("virtual")
        || source.source.to_ascii_lowercase().contains("zwift");
    let chunks = if virtual_activity {
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
