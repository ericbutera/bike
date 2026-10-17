//! Processor identities used by task history.
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProcessorDefinition {
    pub task_type: String,
    pub processing_version: String,
}

impl ProcessorDefinition {
    pub fn for_type(task_type: &str) -> Self {
        Self {
            task_type: task_type.into(),
            processing_version: if task_type == "prepare_heatmap" {
                crate::heatmaps::PROJECTION_VERSION.to_string()
            } else {
                "1".into()
            },
        }
    }
}

/// Workload cohorts are logarithmic count bands, never target IDs or payloads.
pub fn workload_cohort(payload: &serde_json::Value) -> String {
    let data = payload.get("data").unwrap_or(payload);
    let count = ["activities", "segment_ids", "activity_ids"]
        .iter()
        .find_map(|key| data.get(key).and_then(serde_json::Value::as_array).map(Vec::len));
    match count {
        Some(0) => "empty".into(),
        Some(1) => "single".into(),
        Some(n) if n <= 10 => "2-10".into(),
        Some(n) if n <= 100 => "11-100".into(),
        Some(_) => "101+".into(),
        None => "unsized".into(),
    }
}
