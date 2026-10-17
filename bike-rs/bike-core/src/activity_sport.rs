use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
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
    Swim,
    Workout,
}

pub static BIKE_ACTIVITY_SPORT_VALUES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    ActivitySport::CYCLING
        .iter()
        .flat_map(|sport| sport.stored_values().iter().copied())
        .collect()
});

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
        "mountainbike" | "mountainbiking" | "mountainbikeride" | "mtb" | "mtbride" => "mountain_bike".to_string(),
        "indoortrainerride" | "indoortrainer" | "indoorcycling" | "virtualride" => "indoor_trainer_ride".to_string(),
        "roadride" | "roadcycling" | "ride" | "cycling" | "biking" | "bike" => "road_ride".to_string(),
        "gravelride" | "ebikeride" | "emountainbikeride" | "velomobile" | "handcycle" => "ride".to_string(),
        "swim" | "swimming" => "swim".to_string(),
        "workout" => "workout".to_string(),
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
        "ride" => Some(ActivitySport::RoadRide),
        "swim" => Some(ActivitySport::Swim),
        "workout" => Some(ActivitySport::Workout),
        _ => None,
    }
}

pub fn is_bike_activity_sport(value: &str) -> bool {
    activity_sport_from_stored(value).is_some_and(ActivitySport::is_cycling)
}

impl ActivitySport {
    const CYCLING: [Self; 3] = [Self::RoadRide, Self::MountainBike, Self::IndoorTrainerRide];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Walk => "walk",
            Self::Hike => "hike",
            Self::MountainBike => "mountain_bike",
            Self::IndoorTrainerRide => "indoor_trainer_ride",
            Self::RoadRide => "road_ride",
            Self::Swim => "swim",
            Self::Workout => "workout",
        }
    }

    pub fn is_cycling(self) -> bool {
        Self::CYCLING.contains(&self)
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
                "gravelride",
                "ebikeride",
                "emountainbikeride",
                "velomobile",
                "handcycle",
            ],
            Self::Swim => &["swim", "swimming", "Swim", "Swimming"],
            Self::Workout => &["workout", "Workout"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_sports_roundtrip_and_classify_through_backend_enum() {
        for (sport, cycling) in [
            (ActivitySport::Run, false),
            (ActivitySport::Walk, false),
            (ActivitySport::Hike, false),
            (ActivitySport::MountainBike, true),
            (ActivitySport::IndoorTrainerRide, true),
            (ActivitySport::RoadRide, true),
            (ActivitySport::Swim, false),
            (ActivitySport::Workout, false),
        ] {
            assert_eq!(serde_json::to_value(sport).unwrap(), sport.as_str());
            for alias in sport.stored_values() {
                assert_eq!(activity_sport_from_stored(alias), Some(sport));
                assert_eq!(is_bike_activity_sport(alias), cycling);
            }
        }
        for alias in BIKE_ACTIVITY_SPORT_VALUES.iter() {
            assert!(is_bike_activity_sport(alias), "{alias}");
        }
    }

    #[test]
    fn unknown_sports_are_preserved_without_guessing_cycling() {
        assert_eq!(normalize_activity_sport(" Pickleball "), "pickleball");
        assert_eq!(activity_sport_from_stored("Pickleball"), None);
        assert!(!is_bike_activity_sport("Pickleball"));
    }
}
