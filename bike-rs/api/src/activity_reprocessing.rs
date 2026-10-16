use crate::{app_error::AppError, storage::AppStorage};
use bike_core::bike_tasks::{JobReference, Priority, TaskActor, TaskCause};
use chrono::Utc;

/// Application command, independent of the HTTP body's optional defaults.
#[derive(Debug)]
pub struct ReprocessActivityCommand {
    pub activity_id: i32,
    pub command_id: uuid::Uuid,
}

#[derive(Debug)]
pub struct AcceptedReprocessing {
    pub activity_id: i32,
    pub activity_import_id: i32,
    pub user_id: i32,
    pub job: JobReference,
}

pub async fn submit(
    state: &AppStorage,
    actor_id: i32,
    request_id: Option<String>,
    command: ReprocessActivityCommand,
) -> Result<AcceptedReprocessing, AppError> {
    let command_id = command.command_id.to_string();
    let input = bike_core::activity_reprocessing::command(
        &state.db,
        command.activity_id,
        command_id.clone(),
        Priority::NewActivity,
    )
    .await?;
    let user_id = input.source.user_id;
    let import_id = input.source.activity_import_id;
    let cause = TaskCause {
        correlation_id: command_id,
        request_id,
        actor: TaskActor::User(actor_id),
        user_id,
        received_at: Utc::now(),
        trace_context: bike_core::observability::inject_current_trace_context().unwrap_or_default(),
    };
    let job = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        state
            .task_submission
            .reprocess_activity_import(input, cause),
    )
    .await
    .map_err(|_| {
        AppError::internal("Temporal acceptance timed out; retry with the same command_id")
    })?
    .map_err(|error| {
        AppError::internal(format!(
            "Temporal did not acknowledge reprocessing: {error}"
        ))
    })?;
    Ok(AcceptedReprocessing {
        activity_id: command.activity_id,
        activity_import_id: import_id,
        user_id,
        job,
    })
}

#[cfg(test)]
mod tests;
