use crate::{
    entities::{activity_training_analyses, segment_efforts},
    heatmaps::{raster::Tile, service::HeatmapService, types::HeatmapQuery},
    test_support::{
        self,
        trace::{attribute, capture, stage},
    },
};
use sea_orm::{DatabaseBackend, MockDatabase};
use std::collections::BTreeMap;

fn row(key: &str, value: impl Into<sea_orm::Value>) -> BTreeMap<String, sea_orm::Value> {
    [(key.into(), value.into())].into()
}

#[tokio::test]
async fn segment_matching_has_operation_parents_for_both_route_directions() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_results([sea_orm::MockExecResult {
            rows_affected: 0,
            last_insert_id: 0,
        }])
        .into_connection();
    let trace = capture(async {
        crate::segment_support::replace_segment_efforts_for_activity(&db, 7, 42, &[])
            .await
            .unwrap();
    })
    .await;
    let matching = stage(&trace, "segments.match.activity");
    assert_eq!(
        attribute(&matching, "bike.operation"),
        &"segments.match.activity".into()
    );
    assert_eq!(attribute(&matching, "route_point_count"), &0_i64.into());
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<BTreeMap<String, sea_orm::Value>>::new()])
        .append_exec_results([sea_orm::MockExecResult {
            rows_affected: 0,
            last_insert_id: 0,
        }])
        .into_connection();
    let trace = capture(async {
        assert!(
            crate::segment_support::replace_segment_efforts_for_segment(&db, 7, 3, &[])
                .await
                .unwrap()
                .is_empty()
        );
    })
    .await;
    assert_eq!(
        attribute(&stage(&trace, "segments.match.segment"), "bike.operation"),
        &"segments.match.segment".into()
    );
}

#[tokio::test]
async fn heatmap_render_keeps_blocking_queries_under_the_tile_and_reports_cache_hits() {
    let enabled = vec![row("enabled", true)];
    let revision = vec![row("revision", 1_i64)];
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([
            enabled.clone(),
            revision.clone(),
            vec![],
            revision.clone(),
            enabled.clone(),
            revision.clone(),
            enabled.clone(),
            revision.clone(),
            enabled,
            revision,
        ])
        .into_connection();
    let service = HeatmapService::default();
    for hit in [false, true] {
        let trace = capture(async {
            let image = service
                .tile(
                    &db,
                    7,
                    HeatmapQuery {
                        revision: Some("1".into()),
                        ..Default::default()
                    },
                    Tile { z: 0, x: 0, y: 0 },
                )
                .await
                .unwrap();
            assert!(image.png.starts_with(b"\x89PNG"));
        })
        .await;
        let tile = stage(&trace, "heatmap.tile");
        assert_eq!(attribute(&tile, "bike.operation"), &"heatmap.tile".into());
        assert_eq!(attribute(&tile, "cache.hit"), &hit.into());
        assert_eq!(
            attribute(&tile, "bike.db.calls"),
            &(if hit { 4_i64 } else { 6_i64 }).into()
        );
        let spans = trace.spans();
        let render = spans.iter().find(|span| span.name == "heatmap.tile.render");
        if hit {
            assert!(render.is_none());
        } else {
            let render = render.unwrap();
            assert_eq!(render.parent_span_id, tile.span_context.span_id());
            assert_eq!(attribute(render, "bike.db.calls"), &2_i64.into());
            assert_eq!(attribute(render, "chunk_count"), &0_i64.into());
        }
    }
    assert_eq!(db.into_transaction_log().len(), 10);
}

#[tokio::test]
async fn training_rebuild_and_backfill_have_queryable_operation_parents() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([Vec::<activity_training_analyses::Model>::new()])
        .append_query_results([Vec::<crate::entities::activities::Model>::new()])
        .into_connection();
    let trace = capture(async {
        crate::activity_training_analysis::rebuild_activity_training_analysis_cache(&db, &[42])
            .await
            .unwrap();
    })
    .await;
    let training = stage(&trace, "training.rebuild");
    assert_eq!(
        attribute(&training, "bike.operation"),
        &"training.rebuild".into()
    );
    assert_eq!(attribute(&training, "activity_count"), &1_i64.into());
    assert_eq!(attribute(&training, "bike.db.calls"), &3_i64.into());
    let backfill = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<BTreeMap<String, sea_orm::Value>>::new()])
        .into_connection();
    let trace = capture(async {
        assert_eq!(
            crate::activity_training_analysis::backfill_user_activity_training_analysis_cache(
                &backfill, 7
            )
            .await
            .unwrap(),
            0
        );
    })
    .await;
    assert_eq!(
        attribute(&stage(&trace, "training.backfill"), "bike.operation"),
        &"training.backfill".into()
    );
}

#[tokio::test]
async fn cpu_work_has_named_spans_without_logging_activity_bodies() {
    let activity = test_support::activity();
    let trace = capture(async {
        crate::activity_training_analysis::build_activity_training_analysis(&activity);
        tokio::task::spawn_blocking(crate::observability::blocking_with_current_trace(|| {
            crate::heatmaps::preparation::prepare_route(&Default::default())
        }))
        .await
        .unwrap();
        crate::activity_parser::parse_activity_artifact(
            crate::activity_parser::ActivityParserArtifact {
                original_filename: "private-name.fit",
                format: "fit",
                artifact_kind: "original",
                source_quality: "original",
                bytes: include_bytes!("../../tests/fixtures/recording/outdoor.fit"),
            },
        )
        .unwrap();
    })
    .await;
    let spans = trace.spans();
    let request = spans.iter().find(|span| span.name == "request").unwrap();
    for operation in ["training.compute", "heatmap.geometry", "activity.parse"] {
        let span = spans.iter().find(|span| span.name == operation).unwrap();
        assert_eq!(span.parent_span_id, request.span_context.span_id());
        assert_eq!(attribute(span, "bike.operation"), &operation.into());
        assert!(!span
            .attributes
            .iter()
            .any(|field| field.value.to_string().contains("private-name")));
    }
}
