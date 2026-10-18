mod contracts;
mod history;
mod provider;
pub use contracts::*;

use crate::{activity_reprocessing, app_error::AppError, storage::AppStorage};
use bike_core::{
    auth::entities::users,
    bike_tasks::{ActivityTask, FitnessChange, JobReference, Priority, UserPage},
    entities::activities,
    jobs::MaintenanceTask,
};
use std::{future::Future, time::Duration};
use uuid::Uuid;

pub async fn activity(
    state: &AppStorage,
    context: RequestContext,
    activity_id: i32,
    request: ActivityTaskRequest,
) -> Result<AcceptedTask, AppError> {
    validate_command(request.command_id)?;
    let activity = activities::Model::find_by_id(&state.db, activity_id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity not found"))?;
    let record = history::TaskRecord::activity(&context, &activity, &request);
    let result = acknowledged(submit_activity(state, &context, &activity, request)).await;
    record.finish(state, result).await
}

async fn submit_activity(
    state: &AppStorage,
    context: &RequestContext,
    activity: &activities::Model,
    request: ActivityTaskRequest,
) -> Result<JobReference, AppError> {
    let task = ActivityTask {
        user_id: activity.user_id,
        activity_id: activity.id,
        priority: Priority::Background,
    };
    let maintenance = match request.task {
        ActivityAction::Reprocess => {
            return activity_reprocessing::submit(
                state,
                context.actor_id,
                context.request_id.clone(),
                activity_reprocessing::ReprocessActivityCommand {
                    activity_id: activity.id,
                    command_id: request.command_id,
                },
            )
            .await
            .map(|result| result.job)
        }
        ActivityAction::StravaResync => return provider::resync(state, context, activity, request.command_id).await,
        ActivityAction::RebuildFitness => MaintenanceTask::Fitness(FitnessChange {
            user_id: activity.user_id,
            priority: Priority::Background,
        }),
        ActivityAction::RebuildSegments => MaintenanceTask::ActivitySegments(task),
        ActivityAction::RebuildHeatmap => MaintenanceTask::Heatmap(task),
    };
    queue(state, maintenance, request.command_id).await
}

pub async fn user(
    state: &AppStorage,
    context: RequestContext,
    user_id: i32,
    request: UserTaskRequest,
) -> Result<AcceptedTask, AppError> {
    validate_command(request.command_id)?;
    let user = users::Model::find_optional(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("User not found"))?;
    let record = history::TaskRecord::user(&context, user.id, &request);
    let result = acknowledged(submit_user(state, &context, &user, request)).await;
    record.finish(state, result).await
}

async fn submit_user(
    state: &AppStorage,
    context: &RequestContext,
    user: &users::Model,
    request: UserTaskRequest,
) -> Result<JobReference, AppError> {
    let page = UserPage {
        user_id: user.id,
        after_id: 0,
    };
    let maintenance = match request.action {
        UserAction::RebuildFitness => MaintenanceTask::Fitness(FitnessChange {
            user_id: user.id,
            priority: Priority::Background,
        }),
        UserAction::RebuildSegments => MaintenanceTask::UserSegments(page),
        UserAction::ReprocessImports {
            archive_fits_only: false,
        } => MaintenanceTask::UserImports(page),
        UserAction::ReprocessImports {
            archive_fits_only: true,
        } => MaintenanceTask::ArchiveFitImports(page),
        UserAction::BackfillXcTraining => MaintenanceTask::XcTraining(page),
        action => return provider::user(state, context, user, request.command_id, action).await,
    };
    queue(state, maintenance, request.command_id).await
}

async fn queue(state: &AppStorage, task: MaintenanceTask, command_id: Uuid) -> Result<JobReference, AppError> {
    state
        .tasks
        .submit_command(task, command_id)
        .await
        .map_err(|error| AppError::internal(format!("Temporal did not acknowledge the task: {error}")))
}

async fn acknowledged(
    submission: impl Future<Output = Result<JobReference, AppError>>,
) -> Result<JobReference, AppError> {
    tokio::time::timeout(Duration::from_secs(5), submission)
        .await
        .map_err(|_| AppError::internal("Task acceptance timed out. Retry with the same command_id."))?
}

fn validate_command(command_id: Uuid) -> Result<(), AppError> {
    if command_id.is_nil() {
        return Err(AppError::bad_request("A non-empty command_id is required"));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
