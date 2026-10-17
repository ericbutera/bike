use super::{fixture::Fixture, *};
use crate::test_support::trace::{attribute, capture, stage};
use crate::{
    heatmaps::{preparation::prepare_activity, projection::PendingProjection},
    strava::logging::{ExchangeContext, TemporalIds},
};
use opentelemetry_sdk::trace::SpanData;
use sea_orm::MockExecResult;
use std::collections::BTreeMap;

mod sync;

#[tokio::test]
async fn webhook_connection_lookup_belongs_to_intake_even_when_no_owner_is_connected() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<strava_connections::Model>::new()])
        .into_connection();
    let webhook = crate::strava::StravaWebhookEvent {
        object_type: "activity".into(),
        aspect_type: "delete".into(),
        owner_id: 123,
        object_id: 99133701,
        event_time: 1790856000,
        subscription_id: 12345,
        updates: None,
        additional_fields: Default::default(),
    };
    let spans = capture(async {
        assert!(crate::strava::events::accept_webhook(&db, &webhook, None)
            .await
            .unwrap()
            .is_none());
    })
    .await;
    stage(&spans, "strava.webhook.accept");
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn deletion_queries_belong_to_the_record_deletion_stage() {
    let fixture = Fixture::new();
    let segment: BTreeMap<String, sea_orm::Value> = [("segment_id".into(), 3.into())].into();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![segment]])
        .append_exec_results((0..4).map(|_| MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }))
        .into_connection();
    let spans = capture(async {
        let deleted = crate::activity_lifecycle::delete_activity_records(&db, fixture.customer.id, fixture.activity())
            .await
            .unwrap();
        assert_eq!(deleted.segment_ids, [3]);
        assert_eq!(deleted.source, None);
    })
    .await;
    let deletion = stage(&spans, "activity.delete.records");
    assert!(deletion
        .attributes
        .iter()
        .any(|field| { field.key.as_str() == "activity_id" && field.value == 42_i64.into() }));
    assert_eq!(db.into_transaction_log().len(), 5);
}

#[tokio::test]
async fn fitness_stage_includes_input_queries_and_published_rows() {
    let inserted: BTreeMap<String, sea_orm::Value> = [("id".into(), 1.into())].into();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<BTreeMap<String, sea_orm::Value>>::new(), Vec::new()])
        .append_query_results([vec![inserted]])
        .append_exec_results((0..3).map(|_| MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }))
        .into_connection();
    let spans = capture(async {
        crate::analytics::rebuild_fitness_freshness_cache(&db, 7).await.unwrap();
    })
    .await;
    let fitness = stage(&spans, "fitness.rebuild");
    assert_eq!(attribute(&fitness, "bike.operation"), &"fitness.rebuild".into());
    assert_eq!(attribute(&fitness, "activity_count"), &0_i64.into());
    assert_eq!(attribute(&fitness, "day_count"), &1_i64.into());
    let transactions = db.into_transaction_log();
    assert_eq!(transactions.len(), 3);
    let publication = transactions[2].statements();
    let row = publication
        .iter()
        .find(|statement| statement.sql.starts_with("INSERT INTO \"fitness_freshness_daily\""))
        .unwrap();
    assert_eq!(
        &row.values.as_ref().unwrap().0[..7],
        &[
            7.into(),
            chrono::Utc::now().date_naive().into(),
            0.into(),
            0.0.into(),
            0.0.into(),
            0.0.into(),
            0.0.into(),
        ]
    );
    assert!(!publication
        .iter()
        .any(|statement| statement.sql.contains("processing_pipeline_outputs")));
    assert_eq!(publication.last().unwrap().sql, "COMMIT");
}

#[tokio::test]
async fn rebuild_queries_remain_named_when_no_output_is_needed() {
    let segments = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<crate::entities::segments::Model>::new()])
        .into_connection();
    let spans = capture(async {
        crate::analytics::rebuild_segment_analytics_cache(&segments, &[3])
            .await
            .unwrap();
    })
    .await;
    assert_eq!(
        attribute(&stage(&spans, "segments.rebuild"), "bike.operation"),
        &"segments.rebuild".into()
    );
    let activity = MockDatabase::new(DatabaseBackend::Postgres)
        .append_exec_results([MockExecResult {
            rows_affected: 1,
            last_insert_id: 0,
        }])
        .append_query_results([vec![crate::test_support::activity_id_row()]])
        .append_query_results([Vec::<crate::entities::segment_efforts::Model>::new()])
        .into_connection();
    let spans = capture(async {
        crate::analytics::rebuild_activity_analytics_cache(&activity, &[42])
            .await
            .unwrap();
    })
    .await;
    assert_eq!(
        attribute(&stage(&spans, "achievements.rebuild"), "bike.operation"),
        &"achievements.rebuild".into()
    );
    let heatmap = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([Vec::<BTreeMap<String, sea_orm::Value>>::new()])
        .into_connection();
    let spans = capture(async {
        prepare_activity(
            &heatmap,
            PendingProjection {
                activity_id: 42,
                generation: 1,
            },
        )
        .await
        .unwrap();
    })
    .await;
    assert_eq!(
        attribute(&stage(&spans, "heatmap.rebuild"), "bike.operation"),
        &"heatmap.rebuild".into()
    );
}

#[tokio::test]
async fn failed_input_read_stays_under_the_fitness_stage() {
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([sea_orm::DbErr::Custom("fixture read failed".into())])
        .into_connection();
    let spans = capture(async {
        assert!(crate::analytics::rebuild_fitness_freshness_cache(&db, 7).await.is_err());
    })
    .await;
    stage(&spans, "fitness.rebuild");
    assert_eq!(db.into_transaction_log().len(), 1);
}

#[tokio::test]
async fn temporal_attempt_name_identifies_the_step_without_an_extra_span() {
    let fixture = Fixture::new();
    let context = ExchangeContext {
        event: fixture.event,
        temporal: TemporalIds::default(),
    };
    let spans = capture(context.scope("strava_rebuild_fitness", async {
        tokio::task::yield_now().await;
    }))
    .await;
    let spans = spans.spans();
    assert_eq!(spans.len(), 2);
    let request = spans.iter().find(|span| span.name == "request").unwrap();
    let attempt = spans.iter().find(|span| span.name == "strava_rebuild_fitness").unwrap();
    assert_eq!(attempt.parent_span_id, request.span_context.span_id());
    assert!(attempt
        .attributes
        .iter()
        .any(|field| { field.key.as_str() == "processor" && field.value == "strava_rebuild_fitness".into() }));
}
