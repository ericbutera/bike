use super::{BatchCommand, BatchEntry, BatchInput, ManifestScope, Priority, SegmentChange};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegmentSelection {
    All,
    One {
        segment_id: i32,
        expected_revision: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivitySegmentsInput {
    pub command: BatchCommand,
    pub activity_id: i32,
    pub expected_revision: u64,
    pub selection: SegmentSelection,
}

impl BatchEntry for ActivitySegmentsInput {
    fn command_id(&self) -> &str {
        &self.command.command_id
    }
    fn user_id(&self) -> i32 {
        self.command.user_id
    }
    fn priority(&self) -> Priority {
        self.command.priority
    }
    fn belongs_to(&self, scope: &ManifestScope) -> bool {
        match scope {
            ManifestScope::UserSegments { user_id } => {
                self.command.user_id == *user_id && self.selection == SegmentSelection::All
            }
            ManifestScope::Segment {
                segment_id,
                expected_revision,
            } => {
                self.selection
                    == (SegmentSelection::One {
                        segment_id: *segment_id,
                        expected_revision: *expected_revision,
                    })
            }
            _ => false,
        }
    }
}

/// Matching commits the effort set and per-activity analytics together. Global
/// segment analytics is requested separately through its debounce workflow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActivitySegmentsResult {
    pub activity_id: i32,
    pub segments: Vec<SegmentChange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentEffortsInput {
    pub segment_id: i32,
    pub expected_revision: u64,
    pub batch: BatchInput,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentEffortsSelection {
    pub command: BatchCommand,
    pub segment_id: i32,
    pub expected_revision: u64,
}
