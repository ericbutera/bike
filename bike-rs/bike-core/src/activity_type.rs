use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(example = "training")]
pub enum ActivityType {
    Training,
    Race,
}

impl ActivityType {
    pub fn infer(title: &str, filename: &str) -> Self {
        let haystack = format!("{title} {filename}").to_ascii_lowercase();
        if haystack
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|token| matches!(token, "race" | "raceday" | "result" | "results"))
        {
            Self::Race
        } else {
            Self::Training
        }
    }
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
