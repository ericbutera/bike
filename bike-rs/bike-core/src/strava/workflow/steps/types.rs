use crate::strava::objects::ObjectRef;
use serde::{Deserialize, Serialize};

pub use super::super::activity::Effect as ActivityEffect;
pub use super::super::WorkflowTask as Operation;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivitySource {
    pub workflow_id: String,
    pub activity: ObjectRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordingSources {
    pub workflow_id: String,
    pub activity: ObjectRef,
    pub streams: ObjectRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeletionSource {
    pub workflow_id: String,
    pub capture: ObjectRef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mutation {
    pub workflow_id: String,
    pub effect: ActivityEffect,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DerivedWork {
    pub workflow_id: String,
    pub pipeline_run_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Completion {
    pub mutation: Mutation,
    pub derived: DerivedWork,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationResult {
    pub workflow_id: String,
    pub outcome: String,
    pub activity_id: Option<i32>,
    pub import_id: Option<i32>,
}

impl OperationResult {
    pub fn superseded(workflow_id: String) -> Self {
        Self {
            workflow_id,
            outcome: "superseded".into(),
            activity_id: None,
            import_id: None,
        }
    }
}
