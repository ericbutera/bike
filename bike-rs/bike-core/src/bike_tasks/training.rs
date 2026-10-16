use super::{BatchCommand, BatchEntry, BatchResult, FitnessChange, ManifestScope, Priority};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingActivityInput {
    pub command: BatchCommand,
    pub activity_id: i32,
    pub expected_revision: u64,
}

impl BatchEntry for TrainingActivityInput {
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
        matches!(scope, ManifestScope::Training { user_id } if *user_id == self.command.user_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingActivityResult {
    pub fitness: Option<FitnessChange>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingCompletion {
    pub command: BatchCommand,
    pub result: BatchResult,
}
