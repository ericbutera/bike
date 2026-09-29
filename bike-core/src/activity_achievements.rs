use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const STORAGE_FORMAT_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({"segment_id": 7,"segment_title": "Riverfront climb","effort_index": 1,"overall_rank": 3,"personal_rank": 1,"personal_best_duration_seconds": 355}))]
pub struct ActivityAchievementHighlight {
    #[schema(example = 7)]
    pub segment_id: i32,
    #[schema(example = "Riverfront climb")]
    pub segment_title: String,
    #[schema(example = 1)]
    pub effort_index: i32,
    #[schema(example = 3)]
    pub overall_rank: Option<i32>,
    #[schema(example = 1)]
    pub personal_rank: Option<i32>,
    #[schema(example = 355)]
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
