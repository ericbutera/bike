#![cfg(feature = "test-fixtures")]

use bike_core::{
    entities::{
        strava_activity_states,
        strava_events::{self, ActivityChange, ActivityStatus, SourceVersion, StravaEvent, StravaOperation},
    },
    test_support as fixtures,
};
use chrono::Duration;
use migration::{Migrator, MigratorTrait, SchemaManager};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait, Schema};
use serde_json::json;
use uuid::Uuid;

async fn database() -> (DatabaseConnection, DatabaseConnection, String) {
    let url = std::env::var("BIKE_STRAVA_TEST_DATABASE_URL").expect("disposable PostgreSQL URL");
    let admin = Database::connect(&url).await.unwrap();
    let schema = format!("strava_sources_{}", Uuid::new_v4().simple());
    admin
        .execute_unprepared(&format!("CREATE SCHEMA {schema}"))
        .await
        .unwrap();
    let mut options = ConnectOptions::new(url);
    options.set_schema_search_path(&schema);
    let db = Database::connect(options).await.unwrap();
    let definitions = Schema::new(db.get_database_backend());
    db.execute(&definitions.create_table_from_entity(strava_activity_states::Entity))
        .await
        .unwrap();
    db.execute(&definitions.create_table_from_entity(strava_events::Entity))
        .await
        .unwrap();
    (admin, db, schema)
}

fn event() -> StravaEvent {
    let mut event = StravaEvent::new(&fixtures::customer(), &fixtures::connection(), StravaOperation::Delete);
    event.strava_activity_id = Some(fixtures::strava_activity_state().activity_id);
    event.webhook = Some(
        serde_json::from_value(json!({
            "aspect_type":"delete", "object_type":"activity", "event_time":100,
            "object_id":event.activity_id().unwrap(), "owner_id":event.strava_athlete_id,
            "subscription_id":1
        }))
        .unwrap(),
    );
    event
}

#[tokio::test]
#[ignore = "Set BIKE_STRAVA_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn source_writes_keep_provider_time_and_advance_a_clock_regressed_write_token() {
    let (admin, db, schema) = database().await;
    let future = chrono::Utc::now() + Duration::hours(1);
    let initial = strava_activity_states::Model {
        event_time: 100,
        updated_at: future,
        applied_at: future,
        ..fixtures::strava_activity_state()
    };
    let active: strava_activity_states::ActiveModel = initial.clone().into();
    let initial = fixtures::insert(&db, active).await;
    let future = initial.updated_at;
    let mut event = event();
    event.webhook = None;
    event.event_time = i64::MAX;
    event.source_version = Some(SourceVersion {
        updated_at: Some(future),
    });
    assert!(strava_activity_states::Model::matches_version(&db, &event)
        .await
        .unwrap());
    let saved = strava_activity_states::Model::apply_event(&db, &event, None, false)
        .await
        .unwrap();
    assert_eq!(saved.event_time, 100);
    assert_eq!(saved.bike_activity_id, initial.bike_activity_id);
    assert!(saved.updated_at > future);
    assert_eq!(saved.updated_at, saved.applied_at);
    assert!(!strava_activity_states::Model::matches_version(&db, &event)
        .await
        .unwrap());
    event.connection_generation += Duration::seconds(1);
    let reconnected = strava_activity_states::Model::apply_event(&db, &event, None, false)
        .await
        .unwrap();
    assert_eq!(reconnected.event_time, 0);
    assert_eq!(reconnected.bike_activity_id, None);
    assert!(reconnected.updated_at > saved.updated_at);
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "Set BIKE_STRAVA_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn watermark_repair_uses_only_committed_provider_events_from_the_current_generation() {
    let (admin, db, schema) = database().await;
    let state = strava_activity_states::Model {
        event_time: i64::MAX,
        deleted: true,
        bike_activity_id: None,
        deleted_at: Some(fixtures::timestamp()),
        ..fixtures::strava_activity_state()
    };
    let active: strava_activity_states::ActiveModel = state.clone().into();
    fixtures::insert(&db, active).await;
    let original = event();
    for (provider_time, generation, status) in [
        (Some(100), original.connection_generation, ActivityStatus::AlreadyAbsent),
        (Some(200), original.connection_generation, ActivityStatus::Superseded),
        (None, original.connection_generation, ActivityStatus::AlreadyAbsent),
        (
            Some(500),
            original.connection_generation + Duration::seconds(1),
            ActivityStatus::AlreadyAbsent,
        ),
    ] {
        let mut event = original.clone();
        event.strava_event_id = Uuid::new_v4();
        event.connection_generation = generation;
        event.event_time = i64::MAX;
        event.webhook = provider_time.map(|at| {
            let mut webhook = original.webhook.clone().unwrap();
            webhook.event_time = at;
            webhook
        });
        let active: strava_events::ActiveModel = fixtures::strava_event_record(&event).into();
        fixtures::insert(&db, active).await;
        strava_events::Model::record_mutation(&db, &ActivityChange::unchanged(event, status))
            .await
            .unwrap();
    }
    let repair = Migrator::migrations()
        .into_iter()
        .find(|migration| migration.name().contains("strava_provider_watermark"))
        .unwrap();
    repair.up(&SchemaManager::new(&db)).await.unwrap();
    let repaired = strava_activity_states::Entity::find_by_id((state.athlete_id, state.activity_id))
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(repaired.event_time, 100);
    assert!(repaired.deleted);
    assert_eq!(repaired.deleted_at, None);
    assert_eq!(repaired.updated_at, repaired.applied_at);
    assert!(repaired.updated_at > state.updated_at);
    admin
        .execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
