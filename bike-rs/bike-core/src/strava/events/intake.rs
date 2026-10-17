use super::*;
use crate::entities::strava_events;
use sea_orm::DatabaseConnection;
use std::time::Duration;

/// Leaves half a second of Strava's callback budget for HTTP handling/transport.
const WEBHOOK_ACCEPTANCE_TIMEOUT: Duration = Duration::from_millis(1500);

#[async_trait::async_trait]
trait WebhookStore: Send + Sync {
    async fn connection(&self, athlete_id: i64) -> Result<Option<strava_connections::Model>, WorkflowError>;
    async fn customer(&self, user_id: i32) -> Result<users::Model, WorkflowError>;
    async fn accept(&self, event: StravaEvent) -> Result<strava_events::Model, WorkflowError>;
}

#[async_trait::async_trait]
impl WebhookStore for DatabaseConnection {
    async fn connection(&self, athlete_id: i64) -> Result<Option<strava_connections::Model>, WorkflowError> {
        Ok(strava_connections::Model::for_athlete(self, athlete_id).await?)
    }

    async fn customer(&self, user_id: i32) -> Result<users::Model, WorkflowError> {
        Ok(users::Model::find_by_id(self, user_id).await?)
    }

    async fn accept(&self, event: StravaEvent) -> Result<strava_events::Model, WorkflowError> {
        accept(self, event).await
    }
}

struct WebhookIntake {
    subscription_id: Option<i64>,
}

impl WebhookIntake {
    async fn accept(
        &self,
        store: &impl WebhookStore,
        webhook: &StravaWebhookEvent,
        request_id: Option<String>,
    ) -> Result<Option<strava_events::Model>, WorkflowError> {
        tokio::time::timeout(WEBHOOK_ACCEPTANCE_TIMEOUT, self.store_event(store, webhook, request_id))
            .await
            .map_err(|_| WorkflowError::internal("Strava webhook durable acceptance timed out"))?
    }

    async fn store_event(
        &self,
        store: &impl WebhookStore,
        webhook: &StravaWebhookEvent,
        request_id: Option<String>,
    ) -> Result<Option<strava_events::Model>, WorkflowError> {
        let Some(operation) = webhook_operation(webhook) else {
            return Ok(None);
        };
        self.validate(webhook)?;
        let Some(connection) = store.connection(webhook.owner_id).await? else {
            tracing::info!(
                strava_athlete_id = webhook.owner_id,
                strava_activity_id = webhook.object_id,
                strava_event_time = webhook.event_time,
                "Ignored Strava event for an unconnected athlete"
            );
            return Ok(None);
        };
        if is_old_deauthorization(operation, webhook, &connection) {
            tracing::info!(
                strava_athlete_id = webhook.owner_id,
                strava_event_time = webhook.event_time,
                bike_user_id = connection.user_id,
                "Ignored deauthorization from before the current connection"
            );
            return Ok(None);
        }
        let user = store.customer(connection.user_id).await?;
        let mut event = StravaEvent::new(&user, &connection, operation);
        event.validate_connection(&connection)?;
        event.validate_customer(&user)?;
        event.event_time = webhook.event_time;
        event.strava_activity_id = (webhook.object_type == "activity").then_some(webhook.object_id);
        event.webhook = Some(webhook.clone());
        event.request_id = request_id;
        super::super::payload::validate_temporal(&event)?;
        store.accept(event).await.map(Some)
    }

    fn validate(&self, webhook: &StravaWebhookEvent) -> Result<(), WorkflowError> {
        if self
            .subscription_id
            .is_some_and(|expected| expected != webhook.subscription_id)
        {
            return Err(WorkflowError::bad_request("Strava subscription ID does not match"));
        }
        if webhook.owner_id <= 0 || webhook.object_id <= 0 {
            return Err(WorkflowError::bad_request("Invalid Strava athlete or object ID"));
        }
        if webhook.event_time <= 0 || DateTime::from_timestamp(webhook.event_time, 0).is_none() {
            return Err(WorkflowError::bad_request("Invalid Strava event timestamp"));
        }
        if webhook.object_type == "athlete" && webhook.object_id != webhook.owner_id {
            return Err(WorkflowError::bad_request(
                "Strava athlete event does not match its owner",
            ));
        }
        Ok(())
    }
}

fn is_old_deauthorization(
    operation: StravaOperation,
    webhook: &StravaWebhookEvent,
    connection: &strava_connections::Model,
) -> bool {
    operation == StravaOperation::Disconnect && webhook.event_time < connection.created_at.timestamp()
}

#[tracing::instrument(name = "strava.webhook.accept", skip_all, fields(strava_athlete_id = webhook.owner_id, object_type = webhook.object_type, aspect_type = webhook.aspect_type))]
pub async fn accept_webhook(
    db: &DatabaseConnection,
    webhook: &StravaWebhookEvent,
    request_id: Option<String>,
) -> Result<Option<strava_events::Model>, WorkflowError> {
    let subscription_id = std::env::var("STRAVA_SUBSCRIPTION_ID")
        .ok()
        .filter(|value| !value.is_empty())
        .map(|value| value.parse::<i64>())
        .transpose()
        .map_err(|_| WorkflowError::internal("Invalid Strava subscription ID"))?;
    WebhookIntake { subscription_id }.accept(db, webhook, request_id).await
}

#[cfg(test)]
mod tests;

pub async fn request_sync(
    db: &DatabaseConnection,
    user: &users::Model,
    connection: &strava_connections::Model,
) -> Result<strava_events::Model, WorkflowError> {
    accept(db, user_event(user, connection, StravaOperation::Sync)?).await
}

pub async fn request_backfill(
    db: &DatabaseConnection,
    user: &users::Model,
    connection: &strava_connections::Model,
    after: DateTime<Utc>,
    before: DateTime<Utc>,
) -> Result<strava_events::Model, WorkflowError> {
    if before <= after {
        return Err(WorkflowError::bad_request("Backfill must end after it starts"));
    }
    let mut event = user_event(user, connection, StravaOperation::Backfill)?;
    event.window = Some(super::super::native::ActivityWindow {
        after: after.timestamp() - 1,
        before: Some(before.timestamp()),
    });
    accept(db, event).await
}

pub async fn request_disconnect(
    db: &DatabaseConnection,
    user: &users::Model,
    connection: &strava_connections::Model,
) -> Result<strava_events::Model, WorkflowError> {
    let mut event = user_event(user, connection, StravaOperation::Disconnect)?;
    event.revoke_remote = true;
    accept(db, event).await
}

fn user_event(
    user: &users::Model,
    connection: &strava_connections::Model,
    operation: StravaOperation,
) -> Result<StravaEvent, WorkflowError> {
    if matches!(operation, StravaOperation::Sync | StravaOperation::Backfill) {
        super::super::ensure_connection_scopes_allow_activity_import(connection)?;
    }
    let event = StravaEvent::new(user, connection, operation);
    event.validate_connection(connection)?;
    Ok(event)
}

async fn accept(db: &DatabaseConnection, event: StravaEvent) -> Result<strava_events::Model, WorkflowError> {
    super::super::payload::validate_temporal(&event)?;
    let row = strava_events::Model::accept(db, &event).await?;
    tracing::info!(correlation_id = row.correlation_id, strava_event_id = row.id,
        bike_customer_id = %row.customer_id, bike_user_id = row.user_id,
        strava_athlete_id = row.athlete_id, strava_activity_id = row.activity_id,
        temporal_workflow_id = row.workflow_id, "Accepted durable Strava event");
    Ok(row)
}
