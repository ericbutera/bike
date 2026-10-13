use super::*;
use crate::strava::native::{strava_native_unit::fixture::Fixture, ActivityStatus};
use std::{collections::HashMap, sync::Mutex};

struct Store {
    fixture: Fixture,
    connected: bool,
    calls: Mutex<Vec<&'static str>>,
    events: Mutex<HashMap<String, strava_events::Model>>,
}

impl Store {
    fn new() -> Self {
        Self {
            fixture: Fixture::new(),
            connected: true,
            calls: Mutex::new(vec![]),
            events: Mutex::new(HashMap::new()),
        }
    }

    fn webhook(&self, object: &str, aspect: &str) -> StravaWebhookEvent {
        StravaWebhookEvent {
            object_type: object.into(),
            aspect_type: aspect.into(),
            owner_id: self.fixture.connection.athlete_id,
            object_id: if object == "athlete" {
                self.fixture.connection.athlete_id
            } else {
                99133701
            },
            event_time: self.fixture.connection.created_at.timestamp(),
            subscription_id: 12345,
            updates: Some(serde_json::json!({})),
            additional_fields: Default::default(),
        }
    }

    async fn accept(
        &self,
        webhook: &StravaWebhookEvent,
    ) -> Result<Option<strava_events::Model>, WorkflowError> {
        WebhookIntake {
            subscription_id: Some(12345),
        }
        .accept(self, webhook, Some("fixture-request".into()))
        .await
    }
}

#[async_trait::async_trait]
impl WebhookStore for Store {
    async fn connection(
        &self,
        athlete_id: i64,
    ) -> Result<Option<strava_connections::Model>, WorkflowError> {
        self.calls.lock().unwrap().push("connection");
        assert_eq!(athlete_id, self.fixture.connection.athlete_id);
        Ok(self.connected.then(|| self.fixture.connection.clone()))
    }

    async fn customer(&self, user_id: i32) -> Result<users::Model, WorkflowError> {
        self.calls.lock().unwrap().push("customer");
        assert_eq!(user_id, self.fixture.customer.id);
        Ok(self.fixture.customer.clone())
    }

    async fn accept(&self, event: StravaEvent) -> Result<strava_events::Model, WorkflowError> {
        self.calls.lock().unwrap().push("accept");
        let key = event.webhook_key()?;
        let mut events = self.events.lock().unwrap();
        let mut row = self.fixture.row(ActivityStatus::Pending);
        row.id = event.strava_event_id.to_string();
        row.correlation_id = event.correlation_id.to_string();
        row.workflow_id = event.workflow_id();
        row.activity_id = event.strava_activity_id;
        row.event_key = key.clone();
        row.event = serde_json::to_value(event).unwrap();
        Ok(events.entry(key).or_insert(row).clone())
    }
}

#[tokio::test]
async fn supported_unsigned_events_keep_customer_activity_and_request_identity() {
    let store = Store::new();
    for (aspect, expected) in [
        ("create", StravaOperation::Create),
        ("update", StravaOperation::Update),
        ("delete", StravaOperation::Delete),
    ] {
        let row = store
            .accept(&store.webhook("activity", aspect))
            .await
            .unwrap()
            .unwrap();
        let event: StravaEvent = serde_json::from_value(row.event).unwrap();
        assert_eq!(event.operation, expected);
        assert_eq!(event.bike_customer_id, store.fixture.customer.pid);
        assert_eq!(event.strava_activity_id, Some(99133701));
        assert_eq!(event.request_id.as_deref(), Some("fixture-request"));
        assert_eq!(event.workflow_id(), row.workflow_id);
    }
    for authorized in [serde_json::json!("false"), serde_json::json!(false)] {
        let mut webhook = store.webhook("athlete", "update");
        webhook.updates = Some(serde_json::json!({"authorized": authorized}));
        let row = store.accept(&webhook).await.unwrap().unwrap();
        let event: StravaEvent = serde_json::from_value(row.event).unwrap();
        assert_eq!(event.operation, StravaOperation::Disconnect);
        assert_eq!(event.strava_activity_id, None);
        assert!(!event.revoke_remote);
    }
}

#[tokio::test]
async fn unsupported_events_and_unconnected_athletes_are_ignored() {
    let mut store = Store::new();
    assert!(store
        .accept(&store.webhook("athlete", "update"))
        .await
        .unwrap()
        .is_none());
    assert!(store
        .accept(&store.webhook("activity", "unknown"))
        .await
        .unwrap()
        .is_none());
    assert!(store.calls.lock().unwrap().is_empty());
    store.connected = false;
    assert!(store
        .accept(&store.webhook("activity", "create"))
        .await
        .unwrap()
        .is_none());
    assert_eq!(*store.calls.lock().unwrap(), ["connection"]);
}

#[tokio::test]
async fn invalid_subscription_identity_and_timestamp_never_reach_persistence() {
    let store = Store::new();
    let valid = store.webhook("activity", "create");
    let mut invalid = valid.clone();
    invalid.subscription_id += 1;
    assert!(store.accept(&invalid).await.is_err());
    invalid = valid.clone();
    invalid.owner_id = 0;
    assert!(store.accept(&invalid).await.is_err());
    invalid = valid.clone();
    invalid.object_id = -1;
    assert!(store.accept(&invalid).await.is_err());
    for timestamp in [0, -1, i64::MAX] {
        invalid = valid.clone();
        invalid.event_time = timestamp;
        assert!(store.accept(&invalid).await.is_err());
    }
    assert!(store.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn old_deauthorization_is_ignored_and_owner_mismatch_is_rejected() {
    let store = Store::new();
    let mut webhook = store.webhook("athlete", "update");
    webhook.updates = Some(serde_json::json!({"authorized": false}));
    webhook.event_time -= 1;
    assert!(store.accept(&webhook).await.unwrap().is_none());
    assert_eq!(*store.calls.lock().unwrap(), ["connection"]);
    store.calls.lock().unwrap().clear();
    webhook.object_id += 1;
    assert!(store.accept(&webhook).await.is_err());
    assert!(store.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn disconnecting_connection_rejects_creates_but_accepts_deletes() {
    let mut store = Store::new();
    store.fixture.connection.disconnect_requested_at = Some(store.fixture.connection.created_at);
    assert!(store
        .accept(&store.webhook("activity", "create"))
        .await
        .is_err());
    assert!(store.events.lock().unwrap().is_empty());
    assert!(store
        .accept(&store.webhook("activity", "delete"))
        .await
        .unwrap()
        .is_some());
}

#[tokio::test]
async fn duplicate_delivery_reuses_the_event_and_temporal_identity() {
    let store = Store::new();
    let webhook = store.webhook("activity", "update");
    let first = store.accept(&webhook).await.unwrap().unwrap();
    let duplicate = WebhookIntake {
        subscription_id: Some(12345),
    }
    .accept(&store, &webhook, Some("retry-request".into()))
    .await
    .unwrap()
    .unwrap();
    assert_eq!(duplicate.id, first.id);
    assert_eq!(duplicate.correlation_id, first.correlation_id);
    assert_eq!(duplicate.workflow_id, first.workflow_id);
    assert_eq!(duplicate.event["request_id"], "fixture-request");
    assert_eq!(store.events.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn updates_and_reconnections_are_distinct_from_duplicate_delivery() {
    let mut store = Store::new();
    let mut webhook = store.webhook("activity", "update");
    let first = store.accept(&webhook).await.unwrap().unwrap();
    webhook.updates = Some(serde_json::json!({"title": "Changed"}));
    let update = store.accept(&webhook).await.unwrap().unwrap();
    assert_ne!(update.id, first.id);
    store.fixture.connection.created_at += chrono::Duration::seconds(1);
    let reconnect = store.accept(&webhook).await.unwrap().unwrap();
    assert_ne!(reconnect.id, update.id);
    assert_eq!(store.events.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn durable_duplicate_uses_the_existing_event_without_another_audit_insert() {
    use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};
    let store = Store::new();
    let row = store
        .accept(&store.webhook("activity", "update"))
        .await
        .unwrap()
        .unwrap();
    let mut duplicate: StravaEvent = serde_json::from_value(row.event.clone()).unwrap();
    duplicate.strava_event_id = Uuid::new_v4();
    duplicate.request_id = Some("another-delivery".into());
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_results([MockExecResult {
            rows_affected: 0,
            last_insert_id: 0,
        }])
        .append_query_results([vec![row.clone()]])
        .into_connection();
    let accepted = strava_events::Model::accept(&db, &duplicate).await.unwrap();
    assert_eq!(accepted.id, row.id);
    assert_eq!(accepted.workflow_id, row.workflow_id);
    assert_eq!(accepted.event, row.event);
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ON CONFLICT (\\\"event_key\\\") DO NOTHING"));
    assert!(!log.contains("integration_events"));
    assert!(log.contains("COMMIT"));
}

#[tokio::test]
async fn persistence_failure_is_returned_without_accepting_an_event() {
    use sea_orm::{DatabaseBackend, MockDatabase};
    let store = Store::new();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![store.fixture.connection.clone()]])
        .append_query_errors([sea_orm::DbErr::Custom(
            "Fixture customer lookup failed".into(),
        )])
        .into_connection();
    let error = WebhookIntake {
        subscription_id: Some(12345),
    }
    .accept(&db, &store.webhook("activity", "create"), None)
    .await
    .unwrap_err();
    assert_eq!(error.status, axum::http::StatusCode::INTERNAL_SERVER_ERROR);
    assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT"));
}
