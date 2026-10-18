use super::*;
use bike_core::entities::{strava_activity_states, strava_connections};
use integrations::strava::events::{self, ManualOperation, ManualRequest};

async fn connection(state: &AppStorage, user_id: i32) -> Result<strava_connections::Model, AppError> {
    strava_connections::Model::for_user(&state.db, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("This user has no Strava connection"))
}

pub(super) async fn resync(
    state: &AppStorage,
    context: &RequestContext,
    activity: &activities::Model,
    command_id: Uuid,
) -> Result<JobReference, AppError> {
    let user = users::Model::find_optional(&state.db, activity.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Activity owner not found"))?;
    let connection = connection(state, user.id).await?;
    let source = strava_activity_states::Model::for_bike_activity(&state.db, &connection, activity.id)
        .await?
        .ok_or_else(|| AppError::bad_request("Activity has no live mapping to this user's Strava connection"))?;
    events::request_manual(
        &state.strava_intake,
        &user,
        &connection,
        ManualRequest {
            command_id,
            request_id: context.request_id.clone(),
            operation: ManualOperation::Resync {
                bike_activity_id: activity.id,
                strava_activity_id: source.activity_id,
            },
        },
    )
    .await
    .map_err(AppError::from)
}

pub(super) async fn user(
    state: &AppStorage,
    context: &RequestContext,
    user: &users::Model,
    command_id: Uuid,
    action: UserAction,
) -> Result<JobReference, AppError> {
    let operation = match action {
        UserAction::StravaSync => ManualOperation::Sync,
        UserAction::StravaBackfill { after, before } => ManualOperation::Backfill { after, before },
        _ => return Err(AppError::bad_request("Unsupported Strava action")),
    };
    let connection = connection(state, user.id).await?;
    events::request_manual(
        &state.strava_intake,
        user,
        &connection,
        ManualRequest {
            command_id,
            request_id: context.request_id.clone(),
            operation,
        },
    )
    .await
    .map_err(AppError::from)
}
