use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityType {
    Training,
    Race,
}

impl ActivityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Training => "training",
            Self::Race => "race",
        }
    }

    pub fn from_stored(value: &str) -> Self {
        match value {
            "race" => Self::Race,
            _ => Self::Training,
        }
    }

    pub fn is_race(self) -> bool {
        self == Self::Race
    }
}
