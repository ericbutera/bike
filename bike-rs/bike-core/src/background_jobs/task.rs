use serde::Serialize;

/// Marker trait for task types
///
/// This is a simple marker to indicate a type can be used as a task.
/// Tasks must be serializable so they can be stored and transmitted.
pub trait Task: Serialize + Send + Sync {
    /// Get the task type identifier
    fn task_type(&self) -> &'static str;
}
