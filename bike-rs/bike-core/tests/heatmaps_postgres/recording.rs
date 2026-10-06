use super::*;
use bike_core::entities::{activities, heatmap_chunks, heatmap_projections};
use bike_core::{
    activity_data::{deserialize_derived_activity_data, serialize_derived_activity_data},
    activity_details::derive_activity_detail_data,
    heatmaps::{data::HeatmapData, raster::Tile, types::HeatmapQuery},
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};

fn outdoor_route() -> serde_json::Value {
    serde_json::to_value(serialize_derived_activity_data(
        &derive_activity_detail_data(
            "synthetic.fit",
            "fit",
            include_bytes!("../fixtures/recording/outdoor.fit"),
        )
        .unwrap(),
    ))
    .unwrap()
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn recording_summary_backfills_original_evidence_and_cannot_be_overridden() {
    let (db, schema) = legacy_fixture().await;
    let mut route = outdoor_route();
    route["recording"] = serde_json::json!({"environment":"virtual"});
    set_route(&db, 1, route).await;
    db.execute_unprepared(include_str!(
        "../../../migration/src/activity_recording_environment.sql"
    ))
    .await
    .unwrap();
    assert_recording_summary(&db, "virtual").await;
    assert!(db
        .execute_unprepared("UPDATE activities SET recording_environment='outdoor' WHERE id=1")
        .await
        .is_err());
    assert_recording_summary(&db, "virtual").await;
    set_route(&db, 1, outdoor_route()).await;
    assert_recording_summary(&db, "unknown").await;
    let mut route = outdoor_route();
    route["recording"] = serde_json::json!({"environment":"indoor"});
    set_route(&db, 1, route).await;
    assert_recording_summary(&db, "indoor").await;
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

async fn assert_recording_summary(db: &sea_orm::DatabaseConnection, expected: &str) {
    use sea_orm::sea_query::{Alias, Expr};
    let value = activities::Entity::find_by_id(1)
        .select_only()
        .expr(Expr::col((
            activities::Entity,
            Alias::new("recording_environment"),
        )))
        .into_tuple::<String>()
        .one(db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(value, expected);
}

async fn set_route(db: &sea_orm::DatabaseConnection, id: i32, route: serde_json::Value) {
    activities::Entity::update_many()
        .set(activities::ActiveModel {
            derived_data_json: Set(Some(serde_json::from_value(route).unwrap())),
            ..Default::default()
        })
        .filter(activities::Column::Id.eq(id))
        .exec(db)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn retained_context_blocks_tiles_zones_bounds_and_ready_counts_even_with_stale_chunks() {
    let (db, schema) = fixture().await;
    set_route(&db, 1, outdoor_route()).await;
    prepare(&db, 1, 2).await;
    let query = HeatmapQuery::default();
    assert_eq!(
        HeatmapData::progress(&db, 1, &query).await.unwrap().ready,
        1
    );
    let center = geometry::project(-119.999, 45.0);
    let tile = Tile {
        z: 14,
        x: (center[0] * 16384.) as u32,
        y: (center[1] * 16384.) as u32,
    };
    assert!(!HeatmapData::tile_page(&db, 1, &query, tile, (0, -1))
        .await
        .unwrap()
        .is_empty());
    // Simulate a previously published projection still present after metadata repair.
    let mut route = outdoor_route();
    route["recording"] = serde_json::json!({"environment":"virtual"});
    set_route(&db, 1, route).await;
    set_policy(&db, "ready", bike_core::heatmaps::PROJECTION_VERSION)
        .await
        .unwrap();
    let progress = HeatmapData::progress(&db, 1, &query).await.unwrap();
    assert_eq!(progress.ready, 0);
    assert_eq!(progress.min_x, None);
    assert!(HeatmapData::activity_centers(&db, 1, &query)
        .await
        .unwrap()
        .is_empty());
    assert!(HeatmapData::tile_page(&db, 1, &query, tile, (0, -1))
        .await
        .unwrap()
        .is_empty());
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn backfill_recovers_legacy_virtual_copy_and_preserves_real_activity() {
    let (db, schema) = fixture().await;
    let original = derive_activity_detail_data(
        "virtual.fit",
        "fit",
        include_bytes!("../fixtures/recording/virtual.fit"),
    )
    .unwrap();
    let mut copy = original.clone();
    copy.recording = Default::default();
    set_route(
        &db,
        1,
        serde_json::to_value(serialize_derived_activity_data(&copy)).unwrap(),
    )
    .await;
    let started_at = activities::Entity::find_by_id(1)
        .select_only()
        .column(activities::Column::StartedAt)
        .into_tuple::<chrono::DateTime<chrono::Utc>>()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    insert_route(
        &db,
        2,
        "indoor_trainer_ride",
        started_at,
        serde_json::to_value(serialize_derived_activity_data(&original)).unwrap(),
    )
    .await;
    prepare(&db, 1, 2).await;
    assert_eq!(status(&db, 1).await, "pending");
    let stored = activities::Entity::find_by_id(1)
        .select_only()
        .column(activities::Column::DerivedDataJson)
        .into_tuple::<Option<bike_core::activity_data::StoredActivityDerivedData>>()
        .one(&db)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(deserialize_derived_activity_data(Some(&stored))
        .recording
        .excluded());
    prepare(&db, 1, 3).await;
    prepare(&db, 2, 1).await;
    assert_eq!(status(&db, 1).await, "skipped");
    assert_eq!(status(&db, 2).await, "skipped");
    insert_route(
        &db,
        3,
        "ride",
        "2020-01-01T00:00:00Z".parse().unwrap(),
        outdoor_route(),
    )
    .await;
    prepare(&db, 3, 1).await;
    assert_eq!(status(&db, 3).await, "ready");
    let progress = HeatmapData::progress(&db, 1, &HeatmapQuery::default())
        .await
        .unwrap();
    assert_eq!((progress.ready, progress.skipped), (1, 2));
    assert_eq!(
        HeatmapData::activity_centers(&db, 1, &HeatmapQuery::default())
            .await
            .unwrap()
            .len(),
        1
    );
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn policy_upgrade_requeues_chunks_and_rejects_old_worker_publication() {
    let (db, schema) = legacy_fixture().await;
    set_policy(&db, "ready", 3).await.unwrap();
    heatmap_chunks::ActiveModel {
        activity_id: Set(1),
        band: Set(0),
        chunk_index: Set(0),
        min_x: Set(0.1),
        min_y: Set(0.1),
        max_x: Set(0.2),
        max_y: Set(0.2),
        points: Set(geometry::encode(&[[0.1, 0.1], [0.2, 0.2]])),
    }
    .insert(&db)
    .await
    .unwrap();
    db.execute_unprepared(include_str!(
        "../../../migration/src/heatmap_projection_recording_policy.sql"
    ))
    .await
    .unwrap();
    db.execute_unprepared(include_str!(
        "../../../migration/src/heatmap_cycling_sources.sql"
    ))
    .await
    .unwrap();
    assert_eq!(status(&db, 1).await, "pending");
    assert_eq!(heatmap_chunks::Entity::find().count(&db).await.unwrap(), 0);
    assert!(!Projection::pending(&db, 1, 1).await.unwrap());
    for status in ["ready", "skipped"] {
        assert!(set_policy(&db, status, 3).await.is_err());
    }
    assert_eq!(status(&db, 1).await, "pending");
    set_route(&db, 1, outdoor_route()).await;
    prepare(&db, 1, 4).await;
    assert_eq!(status(&db, 1).await, "ready");
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

async fn insert_route(
    db: &sea_orm::DatabaseConnection,
    id: i32,
    sport: &str,
    started_at: chrono::DateTime<chrono::Utc>,
    route: serde_json::Value,
) {
    activities::Entity::insert_many([activities::ActiveModel {
        id: Set(id),
        user_id: Set(1),
        sport: Set(sport.into()),
        source: Set("archive_url_import".into()),
        started_at: Set(started_at),
        derived_data_json: Set(Some(serde_json::from_value(route).unwrap())),
        ..Default::default()
    }])
    .exec_without_returning(db)
    .await
    .unwrap();
}

async fn set_policy(
    db: &sea_orm::DatabaseConnection,
    status: &str,
    version: i32,
) -> Result<sea_orm::UpdateResult, sea_orm::DbErr> {
    heatmap_projections::Entity::update_many()
        .set(heatmap_projections::ActiveModel {
            status: Set(status.into()),
            projection_version: Set(version),
            ..Default::default()
        })
        .filter(heatmap_projections::Column::ActivityId.eq(1))
        .exec(db)
        .await
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn read_filters_apply_the_same_legacy_recording_rules_as_preparation() {
    let (db, schema) = fixture().await;
    set_route(&db, 1, outdoor_route()).await;
    for (sport, source, ready) in [
        ("Ride", "archive_url_import", 1),
        ("indoor trainer", "strava_sync", 0),
        ("Virtual Ride", "strava_sync", 0),
        ("Ride", "trainer_road", 0),
        ("Ride", "trainer-day", 0),
        ("Ride", "Zwift export", 0),
        ("Ride", "virtual-import", 0),
        ("run", "archive_url_import", 0),
        ("hike", "archive_url_import", 0),
        ("swim", "archive_url_import", 0),
    ] {
        activities::Entity::update_many()
            .set(activities::ActiveModel {
                sport: Set(sport.into()),
                source: Set(source.into()),
                ..Default::default()
            })
            .filter(activities::Column::Id.eq(1))
            .exec(&db)
            .await
            .unwrap();
        set_policy(&db, "ready", bike_core::heatmaps::PROJECTION_VERSION)
            .await
            .unwrap();
        assert_eq!(
            HeatmapData::progress(&db, 1, &HeatmapQuery::default())
                .await
                .unwrap()
                .ready,
            ready,
            "{sport}/{source}"
        );
    }
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "Set BIKE_HEATMAP_TEST_DATABASE_URL to a disposable PostgreSQL database"]
async fn cycling_source_policy_blocks_nonrides_and_unavailable_sources_on_all_surfaces() {
    for (sport, format, visible) in [
        ("ride", "fit", true),
        ("run", "fit", false),
        ("hike", "gpx", false),
        ("swim", "fit", false),
        ("ride", "unavailable", false),
    ] {
        assert_source_policy(sport, format, visible).await;
    }
}

async fn assert_source_policy(sport: &str, format: &str, visible: bool) {
    let (db, schema) = fixture().await;
    set_route(&db, 1, outdoor_route()).await;
    activities::Entity::update_many()
        .set(activities::ActiveModel {
            sport: Set(sport.into()),
            format: Set(Some(format.into())),
            ..Default::default()
        })
        .filter(activities::Column::Id.eq(1))
        .exec(&db)
        .await
        .unwrap();
    let generation = heatmap_projections::Entity::find_by_id(1)
        .select_only()
        .column(heatmap_projections::Column::Generation)
        .into_tuple::<i64>()
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    prepare(&db, 1, generation).await;
    let query = HeatmapQuery::default();
    let progress = HeatmapData::progress(&db, 1, &query).await.unwrap();
    assert_eq!(progress.ready > 0, visible);
    assert_eq!(progress.min_x.is_some(), visible);
    assert_eq!(
        !HeatmapData::activity_centers(&db, 1, &query)
            .await
            .unwrap()
            .is_empty(),
        visible
    );
    let center = geometry::project(-119.999, 45.0);
    let tile = Tile {
        z: 14,
        x: (center[0] * 16384.) as u32,
        y: (center[1] * 16384.) as u32,
    };
    assert_eq!(
        !HeatmapData::tile_page(&db, 1, &query, tile, (0, -1))
            .await
            .unwrap()
            .is_empty(),
        visible
    );
    assert_eq!(
        status(&db, 1).await,
        if visible { "ready" } else { "skipped" }
    );
    db.execute_unprepared(&format!("DROP SCHEMA {schema} CASCADE"))
        .await
        .unwrap();
}
