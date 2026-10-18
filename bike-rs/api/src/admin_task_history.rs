use crate::{app_error::AppError, storage::AppStorage};
use bike_core::jobs::history::{TaskExecutionInput, TaskExecutionPage, TaskStatus};
use serde::Deserialize;
use std::{future::Future, time::Duration};
use utoipa::IntoParams;

#[derive(Default, Deserialize, IntoParams)]
pub struct HistoryQuery {
    pub status: Option<TaskStatus>,
    pub page_token: Option<String>,
    pub user_id: Option<i32>,
}

pub async fn list(state: &AppStorage, query: HistoryQuery) -> Result<TaskExecutionPage, AppError> {
    if query.user_id.is_some_and(|id| id <= 0) {
        return Err(AppError::bad_request("User ID must be positive"));
    }
    let token = query.page_token.unwrap_or_default();
    if token.len() > 32768 {
        return Err(AppError::bad_request("Invalid history page token"));
    }
    let token = hex::decode(token).map_err(|_| AppError::bad_request("Invalid history page token"))?;
    read(state.tasks.execution_history(query.status, token, query.user_id)).await
}

pub async fn input(state: &AppStorage, workflow_id: String, run_id: String) -> Result<TaskExecutionInput, AppError> {
    if workflow_id.is_empty() || workflow_id.len() > 1000 {
        return Err(AppError::bad_request("A workflow ID is required"));
    }
    uuid::Uuid::parse_str(&run_id).map_err(|_| AppError::bad_request("A valid run ID is required"))?;
    read(state.tasks.execution_input(workflow_id, run_id)).await
}

async fn read<T>(operation: impl Future<Output = Result<T, bike_core::jobs::native::Error>>) -> Result<T, AppError> {
    match tokio::time::timeout(Duration::from_secs(5), operation).await {
        Ok(Ok(value)) => Ok(value),
        error => {
            if let Ok(Err(error)) = error {
                tracing::error!(%error, "Temporal history read failed");
            }
            Err(AppError {
                status: axum::http::StatusCode::BAD_GATEWAY,
                ..AppError::internal("Task history is unavailable. Try again or open Temporal.")
            })
        }
    }
}
