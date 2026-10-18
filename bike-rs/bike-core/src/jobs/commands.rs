use crate::{
    bike_tasks::{ActivityTask, FitnessChange, UserPage},
    workflow_error::WorkflowError,
};
use serde::Serialize;

/// Named maintenance operations; HTTP callers cannot supply arbitrary workflow names or payloads.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum MaintenanceTask {
    ActivitySegments(ActivityTask),
    Heatmap(ActivityTask),
    Fitness(FitnessChange),
    UserSegments(UserPage),
    UserImports(UserPage),
    ArchiveFitImports(UserPage),
    XcTraining(UserPage),
}

impl MaintenanceTask {
    pub fn workflow_type(&self) -> &'static str {
        match self {
            Self::ActivitySegments(_) => "regenerate_activity_segments",
            Self::Heatmap(_) => "prepare_heatmap",
            Self::Fitness(_) => "rebuild_fitness_freshness",
            Self::UserSegments(_) => "regenerate_user_segments",
            Self::UserImports(_) => "reprocess_user_activity_imports",
            Self::ArchiveFitImports(_) => "reprocess_archive_fit_activity_imports",
            Self::XcTraining(_) => "backfill_user_xc_training",
        }
    }

    pub fn validate(&self) -> Result<(), WorkflowError> {
        let valid = match self {
            Self::ActivitySegments(task) | Self::Heatmap(task) => task.user_id > 0 && task.activity_id > 0,
            Self::Fitness(task) => task.user_id > 0,
            Self::UserSegments(page)
            | Self::UserImports(page)
            | Self::ArchiveFitImports(page)
            | Self::XcTraining(page) => page.user_id > 0 && page.after_id == 0,
        };
        if !valid {
            return Err(WorkflowError::bad_request("A valid maintenance target is required"));
        }
        Ok(())
    }
}
