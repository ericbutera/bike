//! A retained immutable manifest fixes batch membership and entry identities.
use super::{ImportSource, Priority, ReprocessInput};
use crate::{object_store::ObjectRef, workflow_error::WorkflowError};
use serde::{Deserialize, Serialize};

pub const PAGE_SIZE: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchCommand {
    pub command_id: String,
    pub user_id: i32,
    pub priority: Priority,
}

impl BatchCommand {
    pub fn validate(&self) -> Result<(), WorkflowError> {
        if self.command_id.trim().is_empty() {
            return Err(WorkflowError::bad_request("Batch requires a command ID"));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchManifest {
    pub object: ObjectRef,
    pub entries: u64,
    pub command_id: String,
    pub scope: ManifestScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ManifestScope {
    Reprocessing { user_id: i32, selection: ReprocessScope },
    UserSegments { user_id: i32 },
    Training { user_id: i32 },
    Segment { segment_id: i32, expected_revision: u64 },
    Archive { user_id: i32, archive_job_id: i32 },
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchProgress {
    pub manifest: Option<BatchManifest>,
    /// Settled entries only. Continue-As-New never carries a live child.
    pub settled: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchInput {
    pub command: BatchCommand,
    #[serde(default)]
    pub progress: BatchProgress,
}

impl BatchInput {
    pub fn new(command: BatchCommand) -> Self {
        Self {
            command,
            progress: BatchProgress::default(),
        }
    }

    pub fn page_request(&self) -> Result<PageRequest, WorkflowError> {
        self.command.validate()?;
        let manifest = self
            .progress
            .manifest
            .clone()
            .ok_or_else(|| WorkflowError::internal("Batch manifest is not prepared"))?;
        if self.progress.settled > manifest.entries {
            return Err(WorkflowError::bad_request(
                "Batch progress exceeds the retained manifest",
            ));
        }
        Ok(PageRequest {
            command: self.command.clone(),
            manifest,
            offset: self.progress.settled,
        })
    }

    pub fn result(&self) -> BatchResult {
        BatchResult {
            command_id: self.command.command_id.clone(),
            completed: self.progress.settled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    pub command: BatchCommand,
    pub manifest: BatchManifest,
    pub offset: u64,
}

impl PageRequest {
    pub fn expected_entries(&self) -> usize {
        self.manifest.entries.saturating_sub(self.offset).min(PAGE_SIZE as u64) as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchResult {
    pub command_id: String,
    /// Completed source/matching/training operations, not derived readiness.
    pub completed: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReprocessScope {
    UserImports,
    ArchiveFits,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReprocessSelection {
    pub command: BatchCommand,
    pub scope: ReprocessScope,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveCommand {
    pub batch: BatchCommand,
    pub archive_job_id: i32,
    pub source: ObjectRef,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveInput {
    pub archive_job_id: i32,
    pub source: ObjectRef,
    pub batch: BatchInput,
}

pub trait BatchEntry {
    fn command_id(&self) -> &str;
    fn user_id(&self) -> i32;
    fn priority(&self) -> Priority;
    fn belongs_to(&self, scope: &ManifestScope) -> bool;
}

impl BatchEntry for ImportSource {
    fn command_id(&self) -> &str {
        &self.command_id
    }
    fn user_id(&self) -> i32 {
        self.user_id
    }
    fn priority(&self) -> Priority {
        self.priority
    }
    fn belongs_to(&self, scope: &ManifestScope) -> bool {
        matches!(scope, ManifestScope::Archive { user_id, .. } if *user_id == self.user_id)
    }
}

impl BatchEntry for ReprocessInput {
    fn command_id(&self) -> &str {
        &self.source.command_id
    }
    fn user_id(&self) -> i32 {
        self.source.user_id
    }
    fn priority(&self) -> Priority {
        self.source.priority
    }
    fn belongs_to(&self, scope: &ManifestScope) -> bool {
        matches!(scope, ManifestScope::Reprocessing { user_id, .. } if *user_id == self.source.user_id)
    }
}
