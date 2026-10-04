use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Activity sports supported by the activity list filter.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(rename_all = "snake_case", example = "road_ride")]
pub enum ActivitySport {
    Run,
    Walk,
    Hike,
    MountainBike,
    IndoorTrainerRide,
    RoadRide,
}

pub const BIKE_ACTIVITY_SPORT_VALUES: &[&str] = &[
    "ride",
    "Ride",
    "road_ride",
    "road ride",
    "road_cycling",
    "roadcycling",
    "cycling",
    "biking",
    "bike",
    "mountain_bike",
    "mountainbike",
    "mountain_biking",
    "mountain bike",
    "mountain_bike_ride",
    "mountainbikeride",
    "MountainBikeRide",
    "mtb",
    "mtb_ride",
    "indoor_trainer_ride",
    "indoor trainer ride",
    "indoor_cycling",
    "indoorcycling",
    "virtual_ride",
    "virtualride",
    "VirtualRide",
    "gravelride",
    "ebikeride",
    "emountainbikeride",
    "velomobile",
    "handcycle",
];

pub fn normalize_activity_sport(value: &str) -> String {
    let value = value.trim().to_ascii_lowercase();
    let token = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>();

    match token.as_str() {
        "run" | "running" | "virtualrun" | "trailrun" => "run".to_string(),
        "walk" | "walking" => "walk".to_string(),
        "hike" | "hiking" => "hike".to_string(),
        "mountainbike" | "mountainbiking" | "mountainbikeride" | "mtb" | "mtbride" => {
            "mountain_bike".to_string()
        }
        "indoortrainerride" | "indoortrainer" | "indoorcycling" | "virtualride" => {
            "indoor_trainer_ride".to_string()
        }
        "roadride" | "roadcycling" | "ride" | "cycling" | "biking" | "bike" => {
            "road_ride".to_string()
        }
        "gravelride" | "ebikeride" | "emountainbikeride" | "velomobile" | "handcycle" => {
            "ride".to_string()
        }
        "swim" | "swimming" => "swim".to_string(),
        "" => "activity".to_string(),
        _ => value,
    }
}

pub fn activity_sport_from_stored(value: &str) -> Option<ActivitySport> {
    match normalize_activity_sport(value).as_str() {
        "run" => Some(ActivitySport::Run),
        "walk" => Some(ActivitySport::Walk),
        "hike" => Some(ActivitySport::Hike),
        "mountain_bike" => Some(ActivitySport::MountainBike),
        "indoor_trainer_ride" => Some(ActivitySport::IndoorTrainerRide),
        "road_ride" => Some(ActivitySport::RoadRide),
        _ => None,
    }
}

pub fn is_bike_activity_sport(value: &str) -> bool {
    BIKE_ACTIVITY_SPORT_VALUES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(value.trim()))
        || matches!(
            normalize_activity_sport(value).as_str(),
            "road_ride" | "mountain_bike" | "indoor_trainer_ride" | "ride"
        )
}

impl ActivitySport {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Walk => "walk",
            Self::Hike => "hike",
            Self::MountainBike => "mountain_bike",
            Self::IndoorTrainerRide => "indoor_trainer_ride",
            Self::RoadRide => "road_ride",
        }
    }

    /// Values written by older importers are included to keep existing rows filterable.
    pub fn stored_values(self) -> &'static [&'static str] {
        match self {
            Self::Run => &[
                "run",
                "running",
                "virtualrun",
                "trailrun",
                "Run",
                "Running",
                "VirtualRun",
                "TrailRun",
            ],
            Self::Walk => &["walk", "walking", "Walk", "Walking"],
            Self::Hike => &["hike", "hiking", "Hike", "Hiking"],
            Self::MountainBike => &[
                "mountain_bike",
                "mountainbike",
                "mountain_biking",
                "mountain bike",
                "mountain_bike_ride",
                "mountainbikeride",
                "MountainBikeRide",
                "mtb",
                "mtb_ride",
            ],
            Self::IndoorTrainerRide => &[
                "indoor_trainer_ride",
                "indoor trainer ride",
                "indoor_cycling",
                "indoorcycling",
                "virtual_ride",
                "virtualride",
                "VirtualRide",
            ],
            Self::RoadRide => &[
                "road_ride",
                "road ride",
                "road_cycling",
                "roadcycling",
                "ride",
                "Ride",
                "cycling",
                "biking",
                "bike",
            ],
        }
    }
}
