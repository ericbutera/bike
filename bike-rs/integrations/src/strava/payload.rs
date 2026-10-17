//! Budget for Strava values carried in Temporal history, excluding retained objects.
use bike_core::workflow_error::WorkflowError;
use serde::Serialize;

/// Leaves room for SDK metadata below Temporal's 512 KiB warning threshold.
pub const MAX_TEMPORAL_JSON_BYTES: usize = 256 * 1024;

pub fn validate_temporal<T: Serialize>(value: &T) -> Result<(), WorkflowError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| WorkflowError::internal(error.to_string()))?
        .len();
    if bytes > MAX_TEMPORAL_JSON_BYTES {
        return Err(WorkflowError {
            status: axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            message: format!(
                "Strava Temporal JSON is {bytes} bytes; limit is {MAX_TEMPORAL_JSON_BYTES}; retain large data in object storage and pass a reference"
            ),
            errors: None,
            retry_at: None,
        });
    }
    Ok(())
}
