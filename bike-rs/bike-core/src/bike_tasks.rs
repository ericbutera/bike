//! Typed domain contracts shared by Bike task producers and native Activities.
//! Scheduling, timers, and delivery state belong to Temporal.
use crate::{object_store::ObjectRef, workflow_error::WorkflowError};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
mod batch;
mod segments;
mod submission;
mod training;
pub use batch::*;
pub use segments::*;
pub use submission::*;
pub use training::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReprocessInput {
    pub source: ImportSource,
    pub activity_id: i32,
    pub expected_revision: u64,
}

impl ReprocessInput {
    pub fn into_command(self) -> ActivityImport {
        ActivityImport {
            source: self.source,
            target: ImportTarget::ExistingActivity {
                activity_id: self.activity_id,
                expected_revision: self.expected_revision,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    NewActivity,
    Background,
}

/// Retained input and retry-stable command identity, independent of tracing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSource {
    pub command_id: String,
    pub user_id: i32,
    pub activity_import_id: i32,
    pub object: ObjectRef,
    pub priority: Priority,
    /// Captured once by the producer and retained in domain audit history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cause: Option<TaskCause>,
}

impl ImportSource {
    pub fn with_cause(mut self, cause: TaskCause) -> Result<Self, WorkflowError> {
        cause.validate(self.user_id)?;
        self.cause = Some(cause);
        Ok(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportTarget {
    NewActivity,
    ExistingActivity { activity_id: i32, expected_revision: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityImport {
    pub source: ImportSource,
    pub target: ImportTarget,
}

impl ActivityImport {
    pub fn validate_result(&self, result: &ActivityImportResult) -> Result<(), WorkflowError> {
        if result.activity_import_id != self.source.activity_import_id {
            return Err(WorkflowError::conflict(
                "Result does not belong to the requested import",
            ));
        }
        if let ImportTarget::ExistingActivity { activity_id, .. } = self.target {
            if result.activity_id != activity_id {
                return Err(WorkflowError::conflict("Reprocessing changed the activity identity"));
            }
        }
        Ok(())
    }
    pub fn refresh_priority(&self) -> Priority {
        self.source.priority
    }

    pub fn validate(&self) -> Result<(), WorkflowError> {
        if self.source.command_id.trim().is_empty() {
            return Err(WorkflowError::bad_request("Import requires a command ID"));
        }
        if self.source.user_id <= 0 || self.source.activity_import_id <= 0 {
            return Err(WorkflowError::bad_request("Import requires an owner and import ID"));
        }
        if let Some(cause) = &self.source.cause {
            cause.validate(self.source.user_id)?;
        }
        if let ImportTarget::ExistingActivity { expected_revision, .. } = self.target {
            validate_revision(expected_revision)?;
        }
        Ok(())
    }
}

/// Source effects and refresh requests. Revisions help coalescing; each output
/// rebuilds authoritative current data independently of source completion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityImportResult {
    pub activity_id: i32,
    pub activity_import_id: i32,
    pub fitness: FitnessChange,
    pub segments: Vec<SegmentChange>,
    pub heatmap: Option<HeatmapChange>,
}

impl ActivityImportResult {
    /// Refresh urgency is independent of the stored output revision and the
    /// ingestion queue priority. Reprocessing may run at background priority.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.fitness.priority = priority;
        for segment in &mut self.segments {
            segment.priority = priority;
        }
        if let Some(heatmap) = &mut self.heatmap {
            heatmap.priority = priority;
        }
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitnessChange {
    pub user_id: i32,
    pub revision: u64,
    pub dirty_from: NaiveDate,
    pub priority: Priority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentChange {
    pub segment_id: i32,
    pub revision: u64,
    pub priority: Priority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeatmapChange {
    pub user_id: i32,
    pub activity_id: i32,
    pub generation: u64,
    pub priority: Priority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Publication {
    Published,
    Superseded,
}

pub fn validate_revision(revision: u64) -> Result<(), WorkflowError> {
    if revision == 0 {
        return Err(WorkflowError::bad_request("Expected a positive committed revision"));
    }
    Ok(())
}
