//! Opt-in integration checks against a disposable PostgreSQL database.
use bike_core::heatmaps::{
    geometry,
    preparation::prepare_activity,
    projection::{PendingProjection, Projection},
};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DbBackend, Statement};

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn prepares_backfills_and_invalidates_activity_generations() {
    let (db, schema) = fixture().await;
    assert!(!Projection::enabled(&db).await.unwrap());
    db.execute_unprepared("UPDATE feature_flags SET enabled=true")
        .await
        .unwrap();
    assert!(Projection::enabled(&db).await.unwrap());
    db.execute_unprepared(
        "INSERT INTO activities(id,user_id,sport,source) VALUES (4,2,'ride','fixture')",
    )
    .await
    .unwrap();
    assert_eq!(Projection::enqueue_pending(&db).await.unwrap(), 1);
    assert_eq!(Projection::enqueue_pending(&db).await.unwrap(), 0);
    assert_batched_prepare_task(&db).await;
    prepare(&db, 1, 1).await;
    assert_eq!(status(&db, 1).await, "skipped");
    let route = serde_json::json!({"v":2,"route_points":[{"elapsed_seconds":0,"longitude":-85.0,"latitude":45.0},{"elapsed_seconds":10,"longitude":-85.001,"latitude":45.001}]});
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE activities SET derived_data_json=$1 WHERE id=1",
        [route.clone().into()],
    ))
    .await
    .unwrap();
    assert_eq!(status(&db, 1).await, "pending");
    prepare(&db, 1, 1).await;
    assert_eq!(status(&db, 1).await, "pending");
    prepare(&db, 1, 2).await;
    assert_eq!(status(&db, 1).await, "ready");
    let chunk = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT points FROM heatmap_chunks LIMIT 1".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        geometry::decode(&chunk.try_get::<Vec<u8>>("", "points").unwrap())
            .unwrap()
            .len(),
        2
    );
    // Source edits hide published chunks immediately; stale work cannot publish.
    db.execute_unprepared("UPDATE activities SET sport='run' WHERE id=1")
        .await
        .unwrap();
    assert_eq!(status(&db, 1).await, "pending");
    prepare(&db, 1, 2).await;
    assert_eq!(status(&db, 1).await, "pending");
    prepare(&db, 1, 3).await;
    assert_eq!(status(&db, 1).await, "ready");
    db.execute_unprepared("DELETE FROM activities WHERE id=1")
        .await
        .unwrap();
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT count(*)::bigint AS count FROM heatmap_chunks".to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(row.try_get::<i64>("", "count").unwrap(), 0);
    // Deleting an account must also allow activity cascade triggers to finish.
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO activities(id,user_id,sport,source,derived_data_json) VALUES(2,2,'VirtualRide','fixture',$1)",[route.into()])).await.unwrap();
    prepare(&db, 2, 1).await;
    assert_eq!(status(&db, 2).await, "skipped");
    db.execute_unprepared("DELETE FROM users WHERE id=2")
        .await
        .unwrap();
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

async fn assert_batched_prepare_task(db: &sea_orm::DatabaseConnection) {
    let task = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT payload->>'type' AS task_type, jsonb_array_length(payload->'data'->'activities') AS activity_count, payload->'data'->'activities'->0->>'activity_id' AS first_activity_id, payload->'data'->'activities'->0->>'generation' AS first_generation, payload->'data'->'activities'->1->>'activity_id' AS second_activity_id, payload->'data'->'activities'->1->>'generation' AS second_generation FROM background_tasks"
                .to_owned(),
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        task.try_get::<String>("", "task_type").unwrap(),
        "PrepareHeatmap"
    );
    assert_eq!(task.try_get::<i32>("", "activity_count").unwrap(), 2);
    assert_eq!(
        task.try_get::<String>("", "first_activity_id").unwrap(),
        "1"
    );
    assert_eq!(task.try_get::<String>("", "first_generation").unwrap(), "1");
    assert_eq!(
        task.try_get::<String>("", "second_activity_id").unwrap(),
        "4"
    );
    assert_eq!(
        task.try_get::<String>("", "second_generation").unwrap(),
        "1"
    );
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn scopes_tiles_and_metadata_and_invalidates_cached_images() {
    use bike_core::heatmaps::{
        data::HeatmapData,
        service::HeatmapService,
        types::{HeatmapError, HeatmapQuery},
    };
    let (db, schema) = fixture().await;
    let service = HeatmapService::default();
    assert!(matches!(
        service.metadata(&db, 1, HeatmapQuery::default()).await,
        Err(HeatmapError::Disabled)
    ));
    let (query, tile) = ready_tile_query(&db, &service).await;
    let image = service.tile(&db, 1, query.clone(), tile).await.unwrap();
    let cached = service.tile(&db, 1, query.clone(), tile).await.unwrap();
    assert!(std::sync::Arc::ptr_eq(&image.png, &cached.png));
    let other = service
        .metadata(&db, 2, HeatmapQuery::default())
        .await
        .unwrap();
    assert_eq!(other.ready, 1);
    let other_query = HeatmapQuery {
        revision: Some(other.revision),
        ..Default::default()
    };
    assert_eq!(
        service
            .zones(&db, 2, other_query.clone())
            .await
            .unwrap()
            .zones
            .len(),
        1
    );
    let other_image = service
        .tile(&db, 2, other_query.clone(), tile)
        .await
        .unwrap();
    assert_ne!(image.png, other_image.png);
    let rows = HeatmapData::tile_page(&db, 2, &other_query, tile, (0, -1))
        .await
        .unwrap();
    assert!(rows.iter().all(|c| c.activity_id == 2));
    assert!(HeatmapData::tile_page(
        &db,
        1,
        &query,
        bike_core::heatmaps::raster::Tile { z: 14, x: 0, y: 0 },
        (0, -1)
    )
    .await
    .unwrap()
    .is_empty());
    assert_filter_scope(&db, &service, query.clone()).await;
    db.execute_unprepared("DELETE FROM activities WHERE id=1")
        .await
        .unwrap();
    let old = query;
    assert!(matches!(
        service.zones(&db, 1, old.clone()).await,
        Err(HeatmapError::Stale)
    ));
    assert!(matches!(
        service.tile(&db, 1, old, tile).await,
        Err(HeatmapError::Stale)
    ));
    db.execute_unprepared("UPDATE feature_flags SET enabled=false")
        .await
        .unwrap();
    assert!(matches!(
        service.tile(&db, 2, other_query, tile).await,
        Err(HeatmapError::Disabled)
    ));
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

async fn ready_tile_query(
    db: &sea_orm::DatabaseConnection,
    service: &bike_core::heatmaps::service::HeatmapService,
) -> (
    bike_core::heatmaps::types::HeatmapQuery,
    bike_core::heatmaps::raster::Tile,
) {
    use bike_core::heatmaps::{raster::Tile, types::HeatmapQuery};
    db.execute_unprepared("UPDATE feature_flags SET enabled=true")
        .await
        .unwrap();
    seed_routes(db).await;
    let metadata = service
        .metadata(db, 1, HeatmapQuery::default())
        .await
        .unwrap();
    assert_eq!(metadata.ready, 2);
    let query = HeatmapQuery {
        revision: Some(metadata.revision.clone()),
        ..Default::default()
    };
    let zones = service.zones(db, 1, query.clone()).await.unwrap();
    assert_eq!(zones.revision, metadata.revision);
    assert_eq!(zones.zones.len(), 2);
    assert!((zones.zones[0].longitude - -85.0005).abs() < 0.001);
    assert!((zones.zones[0].latitude - 45.0005).abs() < 0.001);
    let center = geometry::project(-85.0005, 45.0005);
    let tile = Tile {
        z: 14,
        x: (center[0] * 16384.0) as u32,
        y: (center[1] * 16384.0) as u32,
    };
    (query, tile)
}

async fn status(db: &sea_orm::DatabaseConnection, id: i32) -> String {
    db.query_one_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "SELECT status FROM heatmap_projections WHERE activity_id=$1",
        [id.into()],
    ))
    .await
    .unwrap()
    .unwrap()
    .try_get("", "status")
    .unwrap()
}

async fn fixture() -> (sea_orm::DatabaseConnection, String) {
    let url = std::env::var("BIKE_HEATMAP_TEST_DATABASE_URL").expect("disposable database URL");
    let db = Database::connect(ConnectOptions::new(url).max_connections(1).to_owned())
        .await
        .unwrap();
    let schema = format!("heatmap_test_{}", uuid::Uuid::new_v4().simple());
    db.execute_unprepared(&format!(
        "CREATE SCHEMA {schema}; SET search_path TO {schema}"
    ))
    .await
    .unwrap();
    db.execute_unprepared(r#"
        CREATE TABLE users(id integer PRIMARY KEY);
        CREATE TABLE activities(id integer PRIMARY KEY, user_id integer REFERENCES users(id) ON DELETE CASCADE, sport text NOT NULL, source text NOT NULL, started_at timestamptz NOT NULL DEFAULT now(), updated_at timestamptz NOT NULL DEFAULT now(), derived_data_json jsonb);
        CREATE TABLE feature_flags(feature_key text PRIMARY KEY, enabled boolean NOT NULL);
        CREATE TABLE background_tasks(id serial PRIMARY KEY,task_type text,payload jsonb,status text,attempts integer,max_attempts integer,created_at timestamptz,updated_at timestamptz);
        INSERT INTO users VALUES (1),(2);
        INSERT INTO feature_flags VALUES ('heatmaps',false);
        INSERT INTO activities(id,user_id,sport,source) VALUES (1,1,'ride','fixture');
    "#).await.unwrap();
    db.execute_unprepared(include_str!("../../migration/src/heatmap_projections.sql"))
        .await
        .unwrap();
    (db, schema)
}

async fn prepare(db: &sea_orm::DatabaseConnection, activity_id: i32, generation: i64) {
    prepare_activity(
        db,
        PendingProjection {
            activity_id,
            generation,
        },
    )
    .await
    .unwrap();
}

async fn seed_routes(db: &sea_orm::DatabaseConnection) {
    let route = serde_json::json!({"v":2,"route_points":[{"elapsed_seconds":0,"longitude":-85.0,"latitude":45.0},{"elapsed_seconds":10,"longitude":-85.001,"latitude":45.001}]});
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE activities SET derived_data_json=$1 WHERE id=1",
        [route.clone().into()],
    ))
    .await
    .unwrap();
    db.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,"INSERT INTO activities(id,user_id,sport,source,derived_data_json,started_at) VALUES(2,2,'ride','fixture',$1,'2026-01-01'),(3,1,'walk','fixture',$1,'2025-01-01')",[route.into()])).await.unwrap();
    prepare(db, 1, 2).await;
    prepare(db, 2, 1).await;
    prepare(db, 3, 1).await;
}

async fn assert_filter_scope(
    db: &sea_orm::DatabaseConnection,
    service: &bike_core::heatmaps::service::HeatmapService,
    mut query: bike_core::heatmaps::types::HeatmapQuery,
) {
    use bike_core::heatmaps::types::HeatmapError;
    query.sport = Some(bike_core::activity_sport::ActivitySport::RoadRide);
    assert_eq!(
        service.metadata(db, 1, query.clone()).await.unwrap().ready,
        1
    );
    assert_eq!(
        service
            .zones(db, 1, query.clone())
            .await
            .unwrap()
            .zones
            .len(),
        1
    );
    query.sport = None;
    query.from = Some("2025-01-01T00:00:00Z".parse().unwrap());
    query.to = Some("2025-01-02T00:00:00Z".parse().unwrap());
    assert_eq!(
        service.metadata(db, 1, query.clone()).await.unwrap().ready,
        1
    );
    assert_eq!(
        service
            .zones(db, 1, query.clone())
            .await
            .unwrap()
            .zones
            .len(),
        1
    );
    query.from = query.to;
    assert!(matches!(
        service.metadata(db, 1, query).await,
        Err(HeatmapError::Invalid(_))
    ));
}
