//! Inputs for named Bike workflows and Activities. Temporal owns execution state.
use crate::{object_store::ObjectRef, workflow_error::WorkflowError};
use serde::{Deserialize, Serialize};
#[cfg(test)]
mod processing;
mod submission;
pub use submission::*;

/// Transport adapters supply retained bytes to the same processing DAG.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActivityProcessingSource {
    Import(Box<ActivityImport>),
    Strava(Box<crate::entities::strava_events::RetainedActivity>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActivityProcessingResult {
    Import(ActivityImportResult),
    Strava(Box<crate::entities::strava_events::ActivityChange>),
}

impl ActivityProcessingSource {
    pub fn validate_result(&self, result: &ActivityProcessingResult) -> Result<(), WorkflowError> {
        match (self, result) {
            (Self::Import(command), ActivityProcessingResult::Import(imported)) => command.validate_result(imported),
            (Self::Strava(source), ActivityProcessingResult::Strava(change)) => {
                let expected = &source.event;
                let actual = &change.event;
                let same_owner = (
                    expected.bike_user_id,
                    expected.bike_customer_id,
                    expected.strava_athlete_id,
                ) == (actual.bike_user_id, actual.bike_customer_id, actual.strava_athlete_id);
                let same_source = (
                    expected.strava_event_id,
                    expected.connection_id,
                    expected.strava_activity_id,
                ) == (actual.strava_event_id, actual.connection_id, actual.strava_activity_id);
                if !same_owner || !same_source {
                    return Err(WorkflowError::conflict(
                        "Result does not belong to the requested provider source",
                    ));
                }
                if !expected.has_same_request(actual) {
                    return Err(WorkflowError::conflict("Result changed its provider request"));
                }
                result.activity()?;
                Ok(())
            }
            _ => Err(WorkflowError::conflict("Processing returned a different source result")),
        }
    }
}
impl ActivityProcessingResult {
    pub fn activity(&self) -> Result<Option<ActivityImportResult>, WorkflowError> {
        use crate::entities::strava_events::ActivityStatus;
        match self {
            Self::Import(imported) => Ok(Some(imported.clone())),
            Self::Strava(change) => {
                if !matches!(change.status, ActivityStatus::Applied | ActivityStatus::Unchanged) {
                    return Ok(None);
                }
                Ok(Some(ActivityImportResult {
                    activity_id: change
                        .bike_activity_id
                        .ok_or_else(|| WorkflowError::invalid_data("Processed source has no activity ID"))?,
                    activity_import_id: change
                        .import_id
                        .ok_or_else(|| WorkflowError::invalid_data("Processed source has no import ID"))?,
                    user_id: change.event.bike_user_id,
                    priority: Priority::NewActivity,
                }))
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityProcessingExecution {
    pub source: ActivityProcessingSource,
    pub workflow: JobReference,
}

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

/// Untagged single input preserves existing Temporal history payloads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SegmentAnalyticsTask {
    Single(SegmentChange),
    Batch { segment_ids: Vec<i32>, priority: Priority },
}

impl SegmentAnalyticsTask {
    pub fn segment_ids(&self) -> Vec<i32> {
        match self {
            Self::Single(task) => vec![task.segment_id],
            Self::Batch { segment_ids, .. } => segment_ids.clone(),
        }
    }

    pub fn priority(&self) -> Priority {
        match self {
            Self::Single(task) => task.priority,
            Self::Batch { priority, .. } => *priority,
        }
    }
}

impl From<SegmentChange> for SegmentAnalyticsTask {
    fn from(task: SegmentChange) -> Self {
        Self::Single(task)
    }
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
    /// Logical object key; never a path on the worker host.
    pub archive_path: String,
    pub offset: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveInput {
    pub job_id: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<i32>,
    #[serde(default)]
    pub cursor: Option<ArchiveCursor>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchivePage {
    pub sources: Vec<ImportSource>,
    pub next_offset: usize,
    pub done: bool,
}
