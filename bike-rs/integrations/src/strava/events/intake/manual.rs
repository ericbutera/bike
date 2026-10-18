use super::*;
use bike_core::bike_tasks::JobReference;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub enum ManualOperation {
    Sync,
    Backfill {
        after: DateTime<Utc>,
        before: DateTime<Utc>,
    },
    Resync {
        bike_activity_id: i32,
        strava_activity_id: i64,
    },
}

pub struct ManualRequest {
    pub command_id: Uuid,
    pub request_id: Option<String>,
    pub operation: ManualOperation,
}

impl ManualRequest {
    fn event(&self, user: &users::Model, connection: &strava_connections::Model) -> Result<StravaEvent, WorkflowError> {
        super::super::super::ensure_connection_scopes_allow_activity_import(connection)?;
        let operation = match self.operation {
            ManualOperation::Sync => StravaOperation::Sync,
            ManualOperation::Backfill { .. } => StravaOperation::Backfill,
            ManualOperation::Resync { .. } => StravaOperation::Upsert,
        };
        let mut event = user_event(user, connection, operation)?;
        event.validate_customer(user)?;
        let identity = serde_json::to_vec(&(user.id, connection.id, connection.created_at, &self.operation))
            .map_err(|error| WorkflowError::internal(error.to_string()))?;
        event.strava_event_id = Uuid::new_v5(&self.command_id, &identity);
        event.correlation_id = event.strava_event_id;
        event.request_id = self.request_id.clone();
        // Manual requests have no provider ordering timestamp.
        event.event_time = 0;
        match self.operation {
            ManualOperation::Backfill { after, before } => {
                if before <= after {
                    return Err(WorkflowError::bad_request("Backfill must end after it starts"));
                }
                event.window = Some(crate::strava::native::ActivityWindow {
                    after: after.timestamp() - 1,
                    before: Some(before.timestamp()),
                });
            }
            ManualOperation::Resync {
                bike_activity_id,
                strava_activity_id,
            } => {
                if bike_activity_id <= 0 || strava_activity_id <= 0 {
                    return Err(WorkflowError::bad_request("A mapped Strava activity is required"));
                }
                event.bike_activity_id = Some(bike_activity_id);
                event.strava_activity_id = Some(strava_activity_id);
            }
            ManualOperation::Sync => {}
        }
        Ok(event)
    }
}

pub async fn request_manual(
    intake: &StravaIntake,
    user: &users::Model,
    connection: &strava_connections::Model,
    request: ManualRequest,
) -> Result<JobReference, WorkflowError> {
    let event = request.event(user, connection)?;
    crate::strava::payload::validate_temporal(&event)?;
    let client = intake
        .client
        .get_or_try_init(|| super::super::submission::connect(&intake.config))
        .await
        .map_err(|error| WorkflowError::internal(format!("Strava workflow connection failed: {error}")))?;
    let row = strava_events::Model::accept(&intake.db, &event).await?;
    super::super::submission::submit_accepted(client, &intake.config, &intake.db, row).await
}

#[cfg(test)]
mod tests;
