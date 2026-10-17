use crate::{activities::DerivedOperations, operations::BikeOperations};
use bike_core::{
    entities::{activities, segment_efforts},
    object_store::ObjectOwner,
    test_support as fixtures,
};
use sea_orm::MockDatabase;

fn operations(mock: MockDatabase) -> BikeOperations {
    let (objects, _) = fixtures::object_storage(ObjectOwner::ActivityImports);
    BikeOperations::new(mock.into_connection(), objects)
}

#[tokio::test]
async fn activity_achievements_rebuild_current_rows_without_execution_records() {
    let mock = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_exec_results([fixtures::written()]);
    operations(mock)
        .rebuild_activity_achievements(fixtures::activity_task())
        .await
        .unwrap();
}

#[tokio::test]
async fn noncycling_training_clears_previous_analysis() {
    let mock = fixtures::postgres_mock()
        .append_query_results([vec![activities::Model {
            sport: "walk".into(),
            ..fixtures::activity()
        }]])
        .append_exec_results([fixtures::written()]);
    operations(mock)
        .rebuild_activity_training(fixtures::activity_task())
        .await
        .unwrap();
}

#[tokio::test]
async fn cycling_training_rebuilds_analysis_from_the_current_activity() {
    let mock = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([Vec::<bike_core::entities::activity_training_analyses::Model>::new()])
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![fixtures::training_analysis()]]);
    operations(mock)
        .rebuild_activity_training(fixtures::activity_task())
        .await
        .unwrap();
}

#[tokio::test]
async fn fitness_rebuilds_daily_rows_without_a_sql_pipeline() {
    let mock = fixtures::postgres_mock()
        .append_query_results([Vec::<bike_core::entities::analytics_user_states::Model>::new()])
        .append_query_results([Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new()])
        .append_query_results([vec![fixtures::activity_id_row()]])
        .append_exec_results([fixtures::written(), fixtures::written()]);
    operations(mock)
        .rebuild_fitness_freshness(fixtures::fitness_change())
        .await
        .unwrap();
}

#[tokio::test]
async fn current_heatmap_needs_no_further_publication() {
    let mock = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::activity()]])
        .append_query_results([vec![bike_core::entities::heatmap_projections::Model {
            status: "skipped".into(),
            published_at: Some(fixtures::timestamp()),
            ..fixtures::heatmap_projection()
        }]])
        .append_query_results([Vec::<std::collections::BTreeMap<String, sea_orm::Value>>::new()]);
    operations(mock)
        .prepare_heatmap(fixtures::activity_task())
        .await
        .unwrap();
}

#[tokio::test]
async fn an_unowned_activity_cannot_start_derived_writes() {
    let mock = fixtures::postgres_mock().append_query_results([Vec::<activities::Model>::new()]);
    assert_eq!(
        operations(mock)
            .rebuild_activity_achievements(fixtures::activity_task())
            .await
            .unwrap_err()
            .status
            .as_u16(),
        404
    );
}

#[tokio::test]
async fn segment_rebuild_without_efforts_clears_old_summaries() {
    let mock = fixtures::postgres_mock()
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([Vec::<segment_efforts::Model>::new()])
        .append_query_results([vec![fixtures::segment()]])
        .append_query_results([vec![bike_core::entities::segment_summaries::Model {
            segment_id: fixtures::segment().id,
            effort_count: 0,
            leader_user_id: None,
            leader_effort_id: None,
            best_duration_seconds: None,
            latest_activity_started_at: None,
            latest_activity_id: None,
            latest_effort_id: None,
            ..fixtures::segment_summary()
        }]])
        .append_exec_results([fixtures::written(), fixtures::written()]);
    operations(mock)
        .rebuild_segment_analytics(bike_core::bike_tasks::SegmentChange {
            segment_id: fixtures::segment().id,
            ..fixtures::segment_change()
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn batch_selection_uses_owned_keyset_pages() {
    use crate::activities::BatchOperations;
    use bike_core::bike_tasks::*;
    for selection in [
        ActivitySelection::Cycling,
        ActivitySelection::Imports {
            archive_fits_only: false,
        },
        ActivitySelection::Imports {
            archive_fits_only: true,
        },
    ] {
        let db = fixtures::postgres_mock().append_query_results([vec![fixtures::activity_id_row()]]);
        let operation = operations(db);
        let tasks = operation
            .activity_page(ActivityPage {
                page: UserPage {
                    user_id: 12,
                    after_id: 30,
                },
                selection,
            })
            .await
            .unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].user_id, 12);
        assert_eq!(tasks[0].priority, Priority::Background);
    }
}

#[tokio::test]
async fn batch_input_rejects_an_unowned_activity_before_reading_its_import() {
    use crate::activities::BatchOperations;
    let operation = operations(fixtures::postgres_mock().append_query_results([Vec::<activities::Model>::new()]));
    assert_eq!(
        operation
            .reprocess_input(fixtures::activity_task())
            .await
            .unwrap_err()
            .status,
        404
    );
}

#[tokio::test]
async fn reconciliation_reads_pending_domain_projections_without_queue_writes() {
    use crate::activities::BatchOperations;
    use std::collections::BTreeMap;
    let enabled: BTreeMap<String, sea_orm::Value> = [("enabled".into(), true.into())].into();
    let projection = fixtures::heatmap_projection();
    let operation = operations(
        fixtures::postgres_mock()
            .append_query_results([vec![enabled]])
            .append_query_results([vec![projection.clone()]]),
    );
    let tasks = operation.pending_heatmaps(0).await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].activity_id, projection.activity_id);
    assert_eq!(tasks[0].user_id, projection.user_id);
}

#[tokio::test]
async fn finished_training_backfill_updates_the_user_preference() {
    use crate::activities::BatchOperations;
    let operation = operations(
        fixtures::postgres_mock()
            .append_query_results([vec![fixtures::user_preferences()], vec![fixtures::user_preferences()]]),
    );
    operation
        .finish_training_backfill(fixtures::customer().id)
        .await
        .unwrap();
}
