use super::{data::HeatmapData, lifecycle::PendingProjection, lifecycle::Projection};
use crate::activity_sport::ActivitySport;
use crate::heatmaps::{geometry, raster::Tile, types::HeatmapQuery};
use crate::test_support;
use sea_orm::{DbBackend, DbErr, MockDatabase, MockExecResult, Value};
use std::collections::BTreeMap;

fn row<const N: usize>(fields: [(&'static str, Value); N]) -> BTreeMap<&'static str, Value> {
    BTreeMap::from(fields)
}

fn filtered_query() -> HeatmapQuery {
    HeatmapQuery {
        sport: Some(ActivitySport::RoadRide),
        from: Some(test_support::timestamp()),
        to: Some(test_support::timestamp() + chrono::Duration::days(1)),
        ..Default::default()
    }
}

#[tokio::test]
async fn progress_maps_counts_and_requires_both_owner_scopes_and_ready_eligibility() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![row([
            ("revision", 9_i64.into()),
            ("ready", 2_i64.into()),
            ("pending", 1_i64.into()),
            ("failed", 0_i64.into()),
            ("skipped", 3_i64.into()),
            ("min_x", Some(0.1).into()),
            ("min_y", Some(0.2).into()),
            ("max_x", Some(0.3).into()),
            ("max_y", Some(0.4).into()),
        ])]])
        .into_connection();
    let progress = HeatmapData::progress(&db, 7, &filtered_query()).await.unwrap();
    assert_eq!((progress.revision, progress.ready, progress.pending), (9, 2, 1));
    assert_eq!((progress.failed, progress.skipped), (0, 3));
    assert_eq!((progress.min_x, progress.max_y), (Some(0.1), Some(0.4)));
    let log = format!("{:?}", db.into_transaction_log());
    for predicate in [
        "activities",
        "heatmap_projections",
        "user_id",
        "recording_environment",
        "started_at",
        "regexp_replace",
        "FILTER",
        "zwift",
        "2026-09-01",
    ] {
        assert!(log.contains(predicate), "missing {predicate}: {log}");
    }
}

#[tokio::test]
async fn missing_progress_is_an_error_and_unknown_revision_defaults_to_zero() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([Vec::<BTreeMap<&str, Value>>::new(), vec![], vec![]])
        .into_connection();
    assert_eq!(HeatmapData::revision(&db, 7).await.unwrap(), 0);
    assert!(HeatmapData::progress(&db, 7, &HeatmapQuery::default())
        .await
        .unwrap_err()
        .to_string()
        .contains("Missing heatmap progress"));
    assert_eq!(HeatmapData::revision(&db, 8).await.unwrap(), 0);
}

#[tokio::test]
async fn centers_and_tiles_keep_filters_cursor_order_and_wrapped_spatial_bounds() {
    let points = geometry::encode(&[[0.0, 0.5], [0.000001, 0.5]]);
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([
            vec![row([("x", 0.0.into()), ("y", 0.5.into())])],
            vec![row([
                ("activity_id", 42.into()),
                ("chunk_index", 64.into()),
                ("points", points.clone().into()),
            ])],
        ])
        .into_connection();
    let centers = HeatmapData::activity_centers(&db, 7, &filtered_query()).await.unwrap();
    assert_eq!((centers[0].x, centers[0].y), (0.0, 0.5));
    let rows = HeatmapData::tile_page(&db, 7, &filtered_query(), Tile { z: 14, x: 0, y: 8192 }, (42, 63))
        .await
        .unwrap();
    assert_eq!((rows[0].activity_id, rows[0].chunk_index), (42, 64));
    assert_eq!(rows[0].points, points);
    let log = format!("{:?}", db.into_transaction_log());
    for predicate in [
        "CASE",
        "projection_version",
        "chunk_index",
        "ORDER BY",
        "LIMIT",
        "box(point",
        "&&",
        "BigUnsigned(Some(64))",
    ] {
        assert!(log.contains(predicate), "missing {predicate}: {log}");
    }
    assert_eq!(Tile { z: 14, x: 0, y: 8192 }.query_bounds().len(), 2);
}

#[tokio::test]
async fn backfill_and_source_reads_keep_generation_and_owner_boundaries() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![row([("activity_id", 42.into()), ("generation", 3_i64.into())])]])
        .append_query_results([vec![test_support::activity()]])
        .append_query_results([vec![row([("activity_id", 42.into())])]])
        .into_connection();
    let pending = Projection::backfill_page(&db, 7, 41).await.unwrap();
    assert_eq!((pending[0].activity_id, pending[0].generation), (42, 3));
    let source = Projection::source(&db, &pending[0]).await.unwrap().unwrap();
    assert_eq!((source.user_id, source.title), (7, "Fixture ride".into()));
    assert!(Projection::pending(&db, 42, 3).await.unwrap());
    let log = format!("{:?}", db.into_transaction_log());
    for predicate in [
        "user_id",
        "generation",
        "projection_version",
        "ORDER BY",
        "BigUnsigned(Some(16))",
    ] {
        assert!(log.contains(predicate), "missing {predicate}: {log}");
    }
}

fn pending() -> PendingProjection {
    PendingProjection {
        activity_id: 42,
        generation: 3,
    }
}

fn chunk(index: usize) -> geometry::Chunk {
    let x = 0.1 + index as f64 / 1000.0;
    geometry::Chunk {
        band: 2,
        bounds: [x, 0.2, x + 0.001, 0.3],
        points: geometry::encode(&[[x, 0.2], [x + 0.001, 0.3]]),
    }
}

#[tokio::test]
async fn publication_batches_chunks_and_atomically_publishes_revision_and_bounds() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![row([("user_id", 7.into())])]])
        .append_exec_results((0..6).map(|_| MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }))
        .into_connection();
    let chunks = (0..65).map(chunk).collect::<Vec<_>>();
    assert!(Projection::publish(&db, &pending(), &chunks).await.unwrap());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("FOR UPDATE"));
    assert!(log.contains("COMMIT"));
    assert!(!log.contains("ROLLBACK"));
    assert_eq!(log.matches("INSERT INTO").count(), 2);
    for evidence in ["heatmap_chunks", "heatmap_user_states", "ready", "Double(Some(0.1))"] {
        assert!(log.contains(evidence), "missing {evidence}: {log}");
    }
    assert!(!log.contains("processing_pipeline_outputs"));
}

#[tokio::test]
async fn obsolete_publication_rolls_back_without_replacing_chunks() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([Vec::<BTreeMap<&str, Value>>::new()])
        .into_connection();
    assert!(!Projection::publish(&db, &pending(), &[chunk(0)]).await.unwrap());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("DELETE"));
    assert!(!log.contains("INSERT"));
}

#[tokio::test]
async fn empty_publication_marks_skipped_and_chunk_failure_rolls_back() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![row([("user_id", 7.into())])]])
        .append_exec_results((0..4).map(|_| MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }))
        .into_connection();
    assert!(Projection::publish(&db, &pending(), &[]).await.unwrap());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("skipped"));
    assert!(log.contains("COMMIT"));
    assert!(!log.contains("INSERT"));

    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![row([("user_id", 7.into())])]])
        .append_exec_results([MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }])
        .append_exec_errors([DbErr::Custom("fixture chunk write failure".into())])
        .into_connection();
    assert!(Projection::publish(&db, &pending(), &[chunk(0)]).await.is_err());
    let log = format!("{:?}", db.into_transaction_log());
    assert!(log.contains("ROLLBACK"));
    assert!(!log.contains("COMMIT"));
    assert!(!log.contains("processing_pipeline_outputs"));
}

#[tokio::test]
async fn failure_and_invalidation_fence_the_generation_and_increment_owner_revision() {
    let db = MockDatabase::new(DbBackend::Postgres)
        .append_query_results([vec![test_support::heatmap_projection()]])
        .append_exec_results((0..5).map(|_| MockExecResult {
            last_insert_id: 0,
            rows_affected: 1,
        }))
        .into_connection();
    Projection::invalidate_owned(&db, 42, 7).await.unwrap();
    Projection::record_failure(&db, &pending()).await.unwrap();
    let log = format!("{:?}", db.into_transaction_log());
    for evidence in [
        "generation",
        "user_id",
        "revision",
        "DELETE",
        "WITH changed AS",
        "generation=$2",
        "Int(Some(42))",
        "BigInt(Some(3))",
    ] {
        assert!(log.contains(evidence), "missing {evidence}: {log}");
    }
}
