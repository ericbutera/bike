//! Immutable Strava events. Temporal owns execution; these rows are an intake outbox.
use super::{objects::checksum, StravaWebhookEvent};
use crate::{auth::entities::users, entities::strava_connections, workflow_error::WorkflowError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
mod intake;
pub use intake::{accept_webhook, request_backfill, request_disconnect, request_sync};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StravaOperation {
    Create,
    Update,
    Upsert,
    Delete,
    Sync,
    Backfill,
    Disconnect,
}

/// The domain row observed before fetching provider data. This is a conditional
/// write token, not provider time or a separate execution record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceVersion {
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StravaEvent {
    pub strava_event_id: Uuid,
    pub correlation_id: Uuid,
    #[serde(default)]
    pub parent_event_id: Option<Uuid>,
    #[serde(default)]
    pub parent_operation: Option<StravaOperation>,
    pub bike_user_id: i32,
    pub bike_customer_id: Uuid,
    pub bike_activity_id: Option<i32>,
    pub strava_athlete_id: i64,
    pub strava_activity_id: Option<i64>,
    pub connection_id: i32,
    pub connection_generation: DateTime<Utc>,
    pub operation: StravaOperation,
    pub event_time: i64,
    pub webhook: Option<StravaWebhookEvent>,
    pub request_id: Option<String>,
    pub trace_context: Option<crate::observability::TraceContextCarrier>,
    pub window: Option<super::native::ActivityWindow>,
    pub revoke_remote: bool,
    #[serde(default)]
    pub progress: Option<super::native::CollectionProgress>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_version: Option<SourceVersion>,
}

impl StravaEvent {
    pub fn provider_event_time(&self) -> Option<i64> {
        self.webhook.as_ref().map(|webhook| webhook.event_time)
    }

    pub fn source_workflow_id(&self) -> Result<String, WorkflowError> {
        Ok(format!(
            "strava-source-user-{}-connection-{}-{}-athlete-{}-activity-{}",
            self.bike_user_id,
            self.connection_id,
            self.connection_generation.timestamp_micros(),
            self.strava_athlete_id,
            self.activity_id()?,
        ))
    }

    pub fn trace_id(&self) -> Option<String> {
        crate::observability::trace_id_from_carrier(self.trace_context.as_ref())
    }

    pub fn activity_id(&self) -> Result<i64, WorkflowError> {
        self.strava_activity_id
            .ok_or_else(|| WorkflowError::bad_request("Strava activity ID is required"))
    }

    pub fn new(user: &users::Model, connection: &strava_connections::Model, operation: StravaOperation) -> Self {
        let id = Uuid::new_v4();
        Self {
            strava_event_id: id,
            correlation_id: id,
            parent_event_id: None,
            parent_operation: None,
            bike_user_id: user.id,
            bike_customer_id: user.pid,
            bike_activity_id: None,
            strava_athlete_id: connection.athlete_id,
            strava_activity_id: None,
            connection_id: connection.id,
            connection_generation: connection.created_at,
            operation,
            event_time: Utc::now().timestamp(),
            webhook: None,
            request_id: None,
            trace_context: crate::observability::inject_current_trace_context(),
            window: None,
            revoke_remote: false,
            progress: None,
            source_version: None,
        }
    }

    pub fn workflow_id(&self) -> String {
        format!("strava-{}", self.strava_event_id)
    }

    pub fn for_activity(&self, id: i64, operation: StravaOperation) -> Self {
        let mut event = self.clone();
        event.parent_event_id = Some(self.strava_event_id);
        event.parent_operation = Some(self.operation);
        event.strava_event_id = Uuid::new_v5(&self.strava_event_id, &id.to_be_bytes());
        event.strava_activity_id = Some(id);
        event.bike_activity_id = None;
        event.operation = operation;
        event.webhook = None;
        event.progress = None;
        event.source_version = None;
        event
    }

    pub fn webhook_key(&self) -> Result<String, WorkflowError> {
        let webhook = self
            .webhook
            .as_ref()
            .ok_or_else(|| WorkflowError::bad_request("Missing Strava webhook event"))?;
        let mut payload = serde_json::to_value((self.connection_generation, webhook))
            .map_err(|error| WorkflowError::bad_request(error.to_string()))?;
        payload.sort_all_objects();
        let bytes = serde_json::to_vec(&payload).map_err(|error| WorkflowError::bad_request(error.to_string()))?;
        Ok(checksum(&bytes))
    }

    pub fn validate_connection(&self, connection: &strava_connections::Model) -> Result<(), WorkflowError> {
        if connection.disconnect_requested_at.is_some()
            && !matches!(self.operation, StravaOperation::Delete | StravaOperation::Disconnect)
        {
            return Err(WorkflowError::bad_request("Strava connection is disconnecting"));
        }
        if !self.is_allowed(connection) {
            return Err(WorkflowError::bad_request(
                "Strava event ownership or connection generation does not match",
            ));
        }
        Ok(())
    }

    pub fn is_allowed(&self, connection: &strava_connections::Model) -> bool {
        self.has_same_connection(connection) && self.has_same_owner(connection)
    }

    fn has_same_connection(&self, connection: &strava_connections::Model) -> bool {
        self.connection_id == connection.id && self.connection_generation == connection.created_at
    }

    fn has_same_owner(&self, connection: &strava_connections::Model) -> bool {
        self.bike_user_id == connection.user_id && self.strava_athlete_id == connection.athlete_id
    }

    pub fn has_same_request(&self, other: &Self) -> bool {
        let same_request = self.has_same_parent(other) && self.has_same_operation(other);
        same_request && self.has_same_execution(other)
    }

    fn has_same_parent(&self, other: &Self) -> bool {
        self.parent_operation == other.parent_operation && self.parent_event_id == other.parent_event_id
    }

    fn has_same_operation(&self, other: &Self) -> bool {
        self.operation == other.operation && self.event_time == other.event_time
    }

    fn has_same_execution(&self, other: &Self) -> bool {
        self.connection_generation == other.connection_generation && self.correlation_id == other.correlation_id
    }

    pub fn validate_customer(&self, customer: &users::Model) -> Result<(), WorkflowError> {
        if customer.id != self.bike_user_id || customer.pid != self.bike_customer_id {
            return Err(WorkflowError::bad_request(
                "Bike customer does not match the Strava event",
            ));
        }
        Ok(())
    }
}

pub fn webhook_operation(event: &StravaWebhookEvent) -> Option<StravaOperation> {
    match (event.object_type.as_str(), event.aspect_type.as_str()) {
        ("activity", "create") => Some(StravaOperation::Create),
        ("activity", "update") => Some(StravaOperation::Update),
        ("activity", "delete") => Some(StravaOperation::Delete),
        ("athlete", "update")
            if event
                .updates
                .as_ref()
                .and_then(|updates| updates.get("authorized"))
                .is_some_and(|value| value == "false" || value == false) =>
        {
            Some(StravaOperation::Disconnect)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> StravaEvent {
        serde_json::from_value(serde_json::json!({
            "strava_event_id":Uuid::new_v4(), "correlation_id":Uuid::new_v4(), "bike_user_id":7,
            "bike_customer_id":Uuid::new_v4(), "bike_activity_id":null, "strava_athlete_id":99, "strava_activity_id":42,
            "connection_id":1, "connection_generation":"2026-10-13T00:00:00Z", "operation":"sync", "event_time":1791849600,
            "webhook":null, "request_id":"incoming-request", "trace_context":null, "window":null, "revoke_remote":false
        })).unwrap()
    }
    #[test]
    fn child_identity_is_stable_and_keeps_customer_and_root_correlation() {
        let parent = event();
        let child = parent.for_activity(42, StravaOperation::Upsert);
        assert_eq!(
            child.strava_event_id,
            parent.for_activity(42, StravaOperation::Upsert).strava_event_id
        );
        assert_ne!(
            child.strava_event_id,
            parent.for_activity(43, StravaOperation::Upsert).strava_event_id
        );
        assert_eq!(child.correlation_id, parent.correlation_id);
        assert_eq!(child.bike_customer_id, parent.bike_customer_id);
        assert_eq!(child.request_id, parent.request_id);
        assert_eq!(child.strava_activity_id, Some(42));
    }

    #[test]
    fn source_identity_spans_events_operations_and_unknown_bike_activity_ids() {
        let original = event();
        let identity = original.source_workflow_id().unwrap();
        let mut changed = original.clone();
        changed.strava_event_id = Uuid::new_v4();
        changed.correlation_id = Uuid::new_v4();
        changed.operation = StravaOperation::Delete;
        changed.event_time += 1;
        changed.bike_activity_id = Some(123);
        assert_eq!(changed.source_workflow_id().unwrap(), identity);
        changed.connection_generation += chrono::Duration::microseconds(1);
        assert_ne!(changed.source_workflow_id().unwrap(), identity);
        changed = original.clone();
        changed.bike_user_id += 1;
        assert_ne!(changed.source_workflow_id().unwrap(), identity);
        changed = original;
        changed.strava_activity_id = None;
        assert!(changed.source_workflow_id().is_err());
    }
    #[test]
    fn webhook_operations_are_explicit() {
        for (object, aspect, updates, expected) in [
            (
                "activity",
                "create",
                serde_json::json!({}),
                Some(StravaOperation::Create),
            ),
            (
                "activity",
                "update",
                serde_json::json!({"title":"Changed", "type":"Ride", "private":"true"}),
                Some(StravaOperation::Update),
            ),
            (
                "activity",
                "delete",
                serde_json::json!({}),
                Some(StravaOperation::Delete),
            ),
            (
                "athlete",
                "update",
                serde_json::json!({"authorized":"false"}),
                Some(StravaOperation::Disconnect),
            ),
            (
                "athlete",
                "update",
                serde_json::json!({"authorized":false}),
                Some(StravaOperation::Disconnect),
            ),
            ("athlete", "update", serde_json::json!({"name":"Changed"}), None),
            ("activity", "unknown", serde_json::json!({}), None),
        ] {
            let webhook = StravaWebhookEvent {
                object_type: object.into(),
                aspect_type: aspect.into(),
                owner_id: 99,
                object_id: 42,
                event_time: 1,
                subscription_id: 2,
                updates: Some(updates),
                additional_fields: Default::default(),
            };
            assert_eq!(webhook_operation(&webhook), expected);
        }
    }
    #[test]
    fn duplicate_webhooks_share_a_key_but_reconnections_do_not() {
        let mut first = event();
        first.webhook = Some(serde_json::from_value(serde_json::json!({"object_type":"activity","aspect_type":"update","owner_id":99,"object_id":42,"event_time":1,"subscription_id":2,"updates":{}})).unwrap());
        let mut duplicate = first.clone();
        duplicate.strava_event_id = Uuid::new_v4();
        assert_eq!(first.webhook_key().unwrap(), duplicate.webhook_key().unwrap());
        duplicate.connection_generation += chrono::Duration::seconds(1);
        assert_ne!(first.webhook_key().unwrap(), duplicate.webhook_key().unwrap());
    }
}
