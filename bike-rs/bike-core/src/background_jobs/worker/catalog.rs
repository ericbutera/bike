//! Shared, versioned processor budgets. The worker publishes its actual registry.
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProcessorDefinition {
    pub task_type: String,
    pub processing_version: String,
    pub runtime_budget_seconds: i64,
    pub progress_budget_seconds: i64,
    pub queue_budget_seconds: i64,
    pub outlier_floor_seconds: f64,
}

impl ProcessorDefinition {
    pub fn for_type(task_type: &str) -> Self {
        let runtime = match task_type {
            "email_notification" => 120,
            "process_activity_import" | "reprocess_activity_import" => 900,
            "prepare_heatmap" | "rebuild_fitness_freshness" => 2700,
            "rebuild_segment_analytics" | "regenerate_segment_efforts" => 3600,
            "activity_archive_import"
            | "reprocess_user_activity_imports"
            | "reprocess_archive_fit_activity_imports"
            | "regenerate_user_segments" => 7200,
            _ => 1800,
        };
        let mut definition = Self {
            task_type: task_type.into(),
            processing_version: if task_type == "prepare_heatmap" {
                crate::heatmaps::PROJECTION_VERSION.to_string()
            } else {
                "1".into()
            },
            runtime_budget_seconds: runtime,
            progress_budget_seconds: runtime.min(900),
            queue_budget_seconds: 1800,
            outlier_floor_seconds: if task_type == "email_notification" {
                5.0
            } else {
                30.0
            },
        };
        // Optional per-processor JSON overrides keep finite labels and defaults.
        // Invalid configuration is rejected at startup by register().
        if let Ok(raw) = std::env::var("WORKER_PROCESSOR_BUDGETS") {
            if let Ok(overrides) = serde_json::from_str::<serde_json::Value>(&raw) {
                if let Some(values) = overrides.get(task_type) {
                    if let Some(seconds) = values["runtime_seconds"].as_i64().filter(|n| *n > 0) {
                        definition.runtime_budget_seconds = seconds;
                    }
                    if let Some(seconds) = values["progress_seconds"].as_i64().filter(|n| *n > 0) {
                        definition.progress_budget_seconds = seconds;
                    }
                    if let Some(seconds) = values["queue_seconds"].as_i64().filter(|n| *n > 0) {
                        definition.queue_budget_seconds = seconds;
                    }
                }
            }
        }
        definition
    }
}

/// Workload cohorts are logarithmic count bands, never target IDs or payloads.
pub fn workload_cohort(payload: &serde_json::Value) -> String {
    let data = payload.get("data").unwrap_or(payload);
    let count = ["activities", "segment_ids", "activity_ids"]
        .iter()
        .find_map(|key| {
            data.get(key)
                .and_then(serde_json::Value::as_array)
                .map(Vec::len)
        });
    match count {
        Some(0) => "empty".into(),
        Some(1) => "single".into(),
        Some(n) if n <= 10 => "2-10".into(),
        Some(n) if n <= 100 => "11-100".into(),
        Some(_) => "101+".into(),
        None => "unsized".into(),
    }
}

pub fn validate_budget_json(raw: &str, task_types: &[String]) -> Result<(), sea_orm::DbErr> {
    let values: std::collections::BTreeMap<String, std::collections::BTreeMap<String, i64>> =
        serde_json::from_str(raw).map_err(|error| {
            sea_orm::DbErr::Custom(format!("Invalid WORKER_PROCESSOR_BUDGETS: {error}"))
        })?;
    for (kind, budgets) in values {
        if !task_types.contains(&kind) {
            return Err(sea_orm::DbErr::Custom(format!(
                "Unknown processor budget: {kind}"
            )));
        }
        for (signal, seconds) in budgets {
            if !["runtime_seconds", "progress_seconds", "queue_seconds"].contains(&signal.as_str())
                || !(1..=86400).contains(&seconds)
            {
                return Err(sea_orm::DbErr::Custom(format!(
                    "Invalid processor budget {kind}.{signal}; use 1-86400 seconds"
                )));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budgets_reject_unknown_processors_signals_and_nonpositive_values() {
        let kinds = vec!["prepare_heatmap".into()];
        assert!(
            validate_budget_json(r#"{"prepare_heatmap":{"runtime_seconds":120}}"#, &kinds).is_ok()
        );
        for raw in [
            r#"{"unknown":{"runtime_seconds":120}}"#,
            r#"{"prepare_heatmap":{"queue_seconds":0}}"#,
            r#"{"prepare_heatmap":{"cpu":1}}"#,
            "null",
        ] {
            assert!(validate_budget_json(raw, &kinds).is_err());
        }
    }
}
