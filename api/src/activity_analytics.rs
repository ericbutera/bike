use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const STORAGE_FORMAT_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ActivityAchievementHighlight {
    pub segment_id: i32,
    pub segment_title: String,
    pub effort_index: i32,
    pub overall_rank: Option<i32>,
    pub personal_rank: Option<i32>,
    pub personal_best_duration_seconds: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct StoredActivityAchievementHighlights {
    #[serde(default = "storage_format_version")]
    pub v: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<ActivityAchievementHighlight>,
}

impl Default for StoredActivityAchievementHighlights {
    fn default() -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            items: Vec::new(),
        }
    }
}

fn storage_format_version() -> u8 {
    STORAGE_FORMAT_VERSION
}

impl StoredActivityAchievementHighlights {
    pub fn from_items(items: Vec<ActivityAchievementHighlight>) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            items,
        }
    }
}