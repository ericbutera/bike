use bike_core::entities::{
    integration_events,
    strava_gateway::{self, Claim, Receipt},
    strava_gateway_bindings as bindings, strava_gateway_receipts as receipts,
};
use bike_core::integration_events_service::{
    list_recent_events, record_event, IntegrationEventListOptions, NewIntegrationEvent,
};
use bike_core::workflow_error::WorkflowError;
use sea_orm::{
    ColumnTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter, Set,
};

struct Fixture {
    db: DatabaseConnection,
    schema: String,
}

impl Fixture {
    async fn new() -> Self {
        let url =
            std::env::var("BIKE_TEST_DATABASE_URL").expect("migrated disposable Bike database URL");
        let db = Database::connect(ConnectOptions::new(url).max_connections(1).to_owned())
            .await
            .unwrap();
        let schema = format!("gateway_events_{}", uuid::Uuid::new_v4().simple());
        db.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; SET search_path TO {schema}"
        ))
        .await
        .unwrap();
        // Use the migrated Bike table shapes while isolating each test's records.
        for table in [
            "strava_gateway_bindings",
            "strava_gateway_receipts",
            "strava_gateway_watermarks",
            "strava_gateway_revocations",
            "integration_events",
        ] {
            db.execute_unprepared(&format!(
                "CREATE TABLE {table} (LIKE public.{table} INCLUDING ALL)"
            ))
            .await
            .unwrap();
        }
        Self { db, schema }
    }

    async fn events(&self, user_id: i32) -> Vec<integration_events::Model> {
        list_recent_events(
            &self.db,
            IntegrationEventListOptions {
                provider: Some("strava".into()),
                user_id: Some(user_id),
                activity_id: None,
                import_id: None,
                limit: 100,
            },
        )
        .await
        .unwrap()
    }

    async fn cleanup(self) {
        self.db
            .execute_unprepared(&format!("DROP SCHEMA {} CASCADE", self.schema))
            .await
            .unwrap();
    }
}

fn receipt(delivery_id: &str) -> Receipt<'_> {
    Receipt {
        delivery_id,
        athlete_id: 887654321,
        user_id: 17,
        activity_id: 9876543210,
        event_time: 100,
        operation: "upsert",
    }
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a migrated disposable PostgreSQL database"]
async fn boundary_events_follow_receipt_retry_and_completion() {
    let fixture = Fixture::new().await;
    let db = &fixture.db;
    let receipt = receipt("rust:events:1");
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Acquired
    );
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Busy
    );
    assert_eq!(fixture.events(17).await.len(), 1);
    strava_gateway::release(
        db,
        &receipt,
        &WorkflowError::bad_request("Invalid Strava activity payload"),
    )
    .await
    .unwrap();
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Acquired
    );
    strava_gateway::complete(db, &receipt).await.unwrap();
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Completed
    );
    let events = fixture.events(17).await;
    assert_eq!(events.len(), 4);
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "gateway.delivery.received")
            .count(),
        2
    );
    let failure = events
        .iter()
        .find(|event| event.event_type == "gateway.delivery.failed")
        .unwrap();
    assert_eq!(failure.level, "error");
    assert_eq!(failure.message, "Invalid Strava activity payload");
    assert_eq!(failure.payload.as_ref().unwrap()["status_code"], 400);
    let applied = events
        .iter()
        .find(|event| event.event_type == "gateway.delivery.applied")
        .unwrap();
    assert_eq!(applied.level, "success");
    assert_eq!(
        applied.payload.as_ref().unwrap()["strava_activity_id"],
        9876543210_i64
    );
    assert_eq!(
        applied.payload.as_ref().unwrap()["delivery_id"],
        "rust:events:1"
    );
    assert!(fixture.events(18).await.is_empty());
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a migrated disposable PostgreSQL database"]
async fn stale_deliveries_are_ignored_once_after_deauthorization() {
    let fixture = Fixture::new().await;
    let db = &fixture.db;
    let receipt = receipt("rust:events:upsert");
    let deauthorize = Receipt {
        delivery_id: "rust:events:2",
        operation: "deauthorize",
        event_time: 200,
        ..receipt
    };
    assert_eq!(
        strava_gateway::claim(db, &deauthorize).await.unwrap(),
        Claim::Acquired
    );
    strava_gateway::complete(db, &deauthorize).await.unwrap();
    let stale = Receipt {
        delivery_id: "rust:events:stale",
        event_time: 150,
        ..receipt
    };
    assert_eq!(
        strava_gateway::claim(db, &stale).await.unwrap(),
        Claim::Completed
    );
    assert_eq!(
        strava_gateway::claim(db, &stale).await.unwrap(),
        Claim::Completed
    );
    let events = fixture.events(17).await;
    assert_eq!(events.len(), 3);
    assert_eq!(
        events
            .iter()
            .filter(|event| event.event_type == "gateway.delivery.ignored")
            .count(),
        1
    );
    assert!(events
        .iter()
        .filter(|event| event.payload.as_ref().unwrap()["operation"] == "deauthorize")
        .all(|event| event.payload.as_ref().unwrap()["strava_activity_id"].is_null()));
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a migrated disposable PostgreSQL database"]
async fn retained_receipts_backfill_once_without_inventing_applied_outcomes() {
    let fixture = Fixture::new().await;
    let receipt = receipt("rust:events:applied");
    strava_gateway::claim(&fixture.db, &receipt).await.unwrap();
    strava_gateway::complete(&fixture.db, &receipt)
        .await
        .unwrap();
    receipts::Entity::insert_many([
        receipts::ActiveModel {
            delivery_id: Set("rust:events:historical".into()),
            athlete_id: Set(887654321),
            activity_id: Set(42),
            event_time: Set(50),
            operation: Set("delete".into()),
            status: Set("completed".into()),
            completed_at: Set(Some("2026-10-01T12:00:00Z".parse().unwrap())),
            ..Default::default()
        },
        receipts::ActiveModel {
            delivery_id: Set("rust:events:pending".into()),
            athlete_id: Set(887654321),
            activity_id: Set(43),
            event_time: Set(150),
            operation: Set("upsert".into()),
            status: Set("processing".into()),
            completed_at: Set(None),
            ..Default::default()
        },
    ])
    .exec_without_returning(&fixture.db)
    .await
    .unwrap();
    for _ in 0..2 {
        fixture
            .db
            .execute_unprepared(include_str!(
                "../../migration/src/strava_gateway_integration_events.sql"
            ))
            .await
            .unwrap();
    }
    let events = fixture.events(17).await;
    assert_eq!(events.len(), 3);
    let historical = events
        .iter()
        .find(|event| event.event_type == "gateway.delivery.completed")
        .unwrap();
    assert_eq!(historical.level, "info");
    assert_eq!(
        historical.created_at.to_rfc3339(),
        "2026-10-01T12:00:00+00:00"
    );
    assert_eq!(
        historical.payload.as_ref().unwrap()["historical_receipt"],
        true
    );
    assert_eq!(
        historical.payload.as_ref().unwrap()["delivery_id"],
        "rust:events:historical"
    );
    assert_eq!(historical.payload.as_ref().unwrap()["operation"], "delete");
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a migrated disposable PostgreSQL database"]
async fn receipt_claim_rolls_back_when_its_event_cannot_be_persisted() {
    let fixture = Fixture::new().await;
    fixture
        .db
        .execute_unprepared(
            "ALTER TABLE integration_events ADD CONSTRAINT reject_received
        CHECK (event_type <> 'gateway.delivery.received')",
        )
        .await
        .unwrap();
    assert!(
        strava_gateway::claim(&fixture.db, &receipt("rust:events:rollback"))
            .await
            .is_err()
    );
    assert_eq!(
        receipts::Entity::find().count(&fixture.db).await.unwrap(),
        0
    );
    assert_eq!(
        bindings::Entity::find().count(&fixture.db).await.unwrap(),
        0
    );
    assert_eq!(
        integration_events::Entity::find()
            .count(&fixture.db)
            .await
            .unwrap(),
        0
    );
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a migrated disposable PostgreSQL database"]
async fn expired_leases_can_be_reclaimed_without_losing_delivery_ownership() {
    let fixture = Fixture::new().await;
    let db = &fixture.db;
    let receipt = receipt("rust:events:expired");
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Acquired
    );
    assert!(strava_gateway::claim(
        db,
        &Receipt {
            user_id: 18,
            ..receipt
        }
    )
    .await
    .is_err());
    assert!(strava_gateway::claim(
        db,
        &Receipt {
            athlete_id: 1,
            ..receipt
        }
    )
    .await
    .is_err());
    let expired = chrono::Utc::now() - chrono::Duration::minutes(3);
    bindings::Entity::update_many()
        .set(bindings::ActiveModel {
            lease_until: Set(Some(expired)),
            ..Default::default()
        })
        .filter(bindings::Column::AthleteId.eq(receipt.athlete_id))
        .exec(db)
        .await
        .unwrap();
    receipts::Entity::update_many()
        .set(receipts::ActiveModel {
            lease_until: Set(Some(expired)),
            ..Default::default()
        })
        .filter(receipts::Column::DeliveryId.eq(receipt.delivery_id))
        .exec(db)
        .await
        .unwrap();
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Acquired
    );
    let wrong_owner = Receipt {
        delivery_id: "rust:events:other",
        ..receipt
    };
    assert!(strava_gateway::complete(db, &wrong_owner).await.is_err());
    assert_eq!(
        bindings::Entity::find_by_id(receipt.athlete_id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .active_delivery_id
            .as_deref(),
        Some(receipt.delivery_id)
    );
    strava_gateway::complete(db, &receipt).await.unwrap();
    assert_eq!(fixture.events(17).await.len(), 3);
    assert_eq!(
        strava_gateway::claim(db, &receipt).await.unwrap(),
        Claim::Completed
    );
    fixture.cleanup().await;
}

#[tokio::test]
#[ignore = "requires BIKE_TEST_DATABASE_URL pointing to a migrated disposable PostgreSQL database"]
async fn event_filters_match_numeric_and_string_bike_ids_without_matching_provider_ids() {
    let fixture = Fixture::new().await;
    for payload in [
        serde_json::json!({"activity_id": 42, "import_id": 7}),
        serde_json::json!({"activity_id": "42", "import_id": "7"}),
        serde_json::json!({"strava_activity_id": 42, "import_id": 7}),
        serde_json::json!({"activity_id": 42, "import_id": 8}),
    ] {
        record_event(
            &fixture.db,
            NewIntegrationEvent {
                user_id: Some(17),
                provider: "strava".into(),
                event_type: "fixture".into(),
                level: "info".into(),
                message: "Fixture event".into(),
                connection_id: None,
                payload: Some(payload),
            },
        )
        .await
        .unwrap();
    }
    let events = list_recent_events(
        &fixture.db,
        IntegrationEventListOptions {
            provider: Some("strava".into()),
            user_id: Some(17),
            activity_id: Some(42),
            import_id: Some(7),
            limit: 100,
        },
    )
    .await
    .unwrap();
    assert_eq!(events.len(), 2);
    fixture.cleanup().await;
}
