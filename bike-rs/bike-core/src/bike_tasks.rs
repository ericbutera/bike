//! Inputs for named Bike workflows and Activities. Temporal owns execution state.
use crate::{object_store::ObjectRef, workflow_error::WorkflowError};
use serde::{Deserialize, Serialize};
mod submission;
pub use submission::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReprocessInput {
    pub source: ImportSource,
    pub activity_id: i32,
}

impl ReprocessInput {
    pub fn into_command(self) -> ActivityImport {
        ActivityImport {
            source: self.source,
            target: ImportTarget::ExistingActivity {
                activity_id: self.activity_id,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Priority {
    NewActivity,
    Background,
}

/// Retained input; retries read and process this file again.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSource {
    pub command_id: String,
    pub user_id: i32,
    pub activity_import_id: i32,
    pub object: ObjectRef,
    pub priority: Priority,
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
    ExistingActivity { activity_id: i32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityImport {
    pub source: ImportSource,
    pub target: ImportTarget,
}

impl ActivityImport {
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
        if let ImportTarget::ExistingActivity { activity_id } = self.target {
            if activity_id <= 0 {
                return Err(WorkflowError::bad_request("Reprocessing requires an activity ID"));
            }
        }
        Ok(())
    }

    pub fn validate_result(&self, result: &ActivityImportResult) -> Result<(), WorkflowError> {
        if result.activity_import_id != self.source.activity_import_id || result.user_id != self.source.user_id {
            return Err(WorkflowError::conflict(
                "Result does not belong to the requested import",
            ));
        }
        if let ImportTarget::ExistingActivity { activity_id } = self.target {
            if result.activity_id != activity_id {
                return Err(WorkflowError::conflict("Reprocessing changed the activity identity"));
            }
        }
        Ok(())
    }
}

/// Saved activity identity. Derived work reads current domain data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityImportResult {
    pub activity_id: i32,
    pub activity_import_id: i32,
    pub user_id: i32,
    pub priority: Priority,
}

impl ActivityImportResult {
    pub fn activity(&self) -> ActivityTask {
        ActivityTask {
            user_id: self.user_id,
            activity_id: self.activity_id,
            priority: self.priority,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityTask {
    pub user_id: i32,
    pub activity_id: i32,
    pub priority: Priority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitnessChange {
    pub user_id: i32,
    pub priority: Priority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentChange {
    pub segment_id: i32,
    pub priority: Priority,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPage {
    pub user_id: i32,
    #[serde(default)]
    pub after_id: i32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivitySelection {
    Cycling,
    Imports { archive_fits_only: bool },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivityPage {
    pub page: UserPage,
    pub selection: ActivitySelection,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveCursor {
    pub job_id: i32,
    pub archive_path: String,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveInput {
    pub job_id: i32,
    #[serde(default)]
    pub cursor: Option<ArchiveCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivePage {
    pub sources: Vec<ImportSource>,
    pub next_offset: usize,
    pub done: bool,
}
