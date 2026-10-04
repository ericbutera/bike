use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Task not found")]
    NotFound,

    #[error("Task processing error: {0}")]
    Processing(String),

    #[error("Max attempts reached")]
    MaxAttemptsReached,
}
