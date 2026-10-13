use super::*;
use axum::extract::FromRequestParts;
use axum::http::Request;
use bike_core::auth::entities::users;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, Schema, Set};
use serde_json::json;
use uuid::Uuid;

struct Fixture {
    state: Arc<AppStorage>,
}

impl Fixture {
    async fn new() -> Self {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let schema = Schema::new(db.get_database_backend());
        for table in [
            schema.create_table_from_entity(users::Entity),
            schema.create_table_from_entity(integration_events::Entity),
        ] {
            db.execute(&table).await.unwrap();
        }
        let pid = Uuid::new_v4();
        users::ActiveModel {
            id: Set(1),
            pid: Set(pid),
            email: Set("history@example.test".into()),
            name: Set("History rider".into()),
            api_key: Set("fixture-key".into()),
            disabled: Set(false),
            created_at: Set(Utc::now()),
            updated_at: Set(Utc::now()),
            ..Default::default()
        }
        .insert(&db)
        .await
        .unwrap();
        let mut state = AppStorage::for_test(db);
        state.local_admin_user_pid = Some(pid);
        Self {
            state: Arc::new(state),
        }
    }

    async fn record(&self, event: integration_events::NewEvent) -> integration_events::Model {
        integration_events::Entity::record(&self.state.db, event)
            .await
            .unwrap()
    }

    async fn history(&self) -> Vec<IntegrationEventResponse> {
        let (mut parts, ()) = Request::new(()).into_parts();
        let context = UserContext::<AppStorage>::from_request_parts(&mut parts, &self.state)
            .await
            .unwrap();
        list_strava_history(context, State(self.state.clone()))
            .await
            .unwrap()
            .0
    }
}

fn event(event_type: &str, payload: serde_json::Value) -> integration_events::NewEvent {
    integration_events::NewEvent {
        user_id: Some(1),
        provider: "strava".into(),
        event_type: event_type.into(),
        level: "info".into(),
        message: "Internal diagnostic".into(),
        connection_id: Some(7),
        payload: Some(payload),
    }
}

#[tokio::test]
async fn account_history_returns_root_events_despite_child_event_noise() {
    let fixture = Fixture::new().await;
    let accepted = "strava.event.accepted";
    let queued = fixture
        .record(event(
            accepted,
            json!({
                "operation":"sync", "temporal_workflow_id":"strava-current"
            }),
        ))
        .await;
    let mut foreign = event(accepted, json!({}));
    foreign.user_id = Some(2);
    fixture.record(foreign).await;
    let mut other = event(accepted, json!({}));
    other.provider = "other".into();
    fixture.record(other).await;
    for _ in 0..30 {
        fixture
            .record(event(
                accepted,
                json!({
                    "operation":"upsert", "parent_event_id":"strava-current"
                }),
            ))
            .await;
    }
    let rows = fixture.history().await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, queued.id);
    assert_eq!(rows[0].payload, queued.payload);
    assert_eq!(rows[0].connection_id, Some(7));
    assert_eq!(rows[0].created_at, queued.created_at);
}

#[tokio::test]
async fn account_history_limits_meaningful_events_to_the_newest_twenty_five() {
    let fixture = Fixture::new().await;
    let mut ids = Vec::new();
    for _ in 0..30 {
        ids.push(
            fixture
                .record(event("strava.event.accepted", json!({"operation":"sync"})))
                .await
                .id,
        );
    }
    let rows = fixture.history().await;
    assert_eq!(rows.len(), 25);
    assert_eq!(
        rows.iter().map(|row| row.id).collect::<Vec<_>>(),
        ids.into_iter().rev().take(25).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn account_history_without_events_returns_an_empty_list() {
    let fixture = Fixture::new().await;
    assert!(fixture.history().await.is_empty());
}
