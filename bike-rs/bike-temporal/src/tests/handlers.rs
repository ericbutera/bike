use super::fixtures::{assert_retryable, environment, Operations};
use crate::activities::DerivedActivities;
use bike_core::test_support as fixtures;
use std::sync::Arc;

#[tokio::test]
async fn named_derived_handlers_call_the_domain_with_the_owned_target() {
    let operations = Arc::new(Operations::default());
    let environment = environment(operations.clone());
    let activity = fixtures::activity_task();
    assert_eq!(
        environment
            .run(DerivedActivities::rebuild_activity_segments, activity.clone())
            .await
            .unwrap(),
        vec![18]
    );
    environment
        .run(DerivedActivities::rebuild_activity_achievements, activity.clone())
        .await
        .unwrap();
    environment
        .run(DerivedActivities::prepare_heatmap, activity.clone())
        .await
        .unwrap();
    environment
        .run(DerivedActivities::rebuild_activity_training, activity.clone())
        .await
        .unwrap();
    environment
        .run(DerivedActivities::rebuild_fitness_freshness, fixtures::fitness_change())
        .await
        .unwrap();
    environment
        .run(
            DerivedActivities::rebuild_segment_analytics,
            fixtures::segment_change().into(),
        )
        .await
        .unwrap();
    let calls = operations.received.lock().unwrap();
    assert_eq!(calls.len(), 6);
    for (_, input) in &calls[..4] {
        assert_eq!(*input, serde_json::to_value(&activity).unwrap());
    }
}

#[tokio::test]
async fn transient_derived_failures_remain_native_activity_failures() {
    let environment = environment(Arc::new(Operations {
        fail: true,
        ..Default::default()
    }));
    let error = environment
        .run(DerivedActivities::rebuild_activity_segments, fixtures::activity_task())
        .await
        .unwrap_err();
    assert_retryable(&error);
}

#[tokio::test]
async fn batch_handlers_forward_owned_inputs_to_the_domain() {
    use crate::activities::BatchActivities;
    use bike_core::bike_tasks::*;
    let operations = Arc::new(Operations::default());
    let environment = environment(operations.clone());
    let page = ActivityPage {
        page: UserPage {
            user_id: 7,
            after_id: 42,
        },
        selection: ActivitySelection::Cycling,
    };
    assert_eq!(
        environment.run(BatchActivities::activity_page, page).await.unwrap(),
        vec![fixtures::activity_task()]
    );
    assert_eq!(
        environment
            .run(BatchActivities::reprocess_input, fixtures::activity_task())
            .await
            .unwrap(),
        fixtures::reprocess_input()
    );
    environment
        .run(BatchActivities::regenerate_segment_efforts, 18)
        .await
        .unwrap();
    environment
        .run(BatchActivities::finish_training_backfill, 7)
        .await
        .unwrap();
    let cursor = environment.run(BatchActivities::prepare_archive, 7).await.unwrap();
    assert_eq!(cursor.job_id, 7);
    let page = environment.run(BatchActivities::archive_page, cursor).await.unwrap();
    assert_eq!(page.sources, vec![fixtures::import_command().source]);
    environment.run(BatchActivities::finish_archive, 7).await.unwrap();
    assert_eq!(
        environment.run(BatchActivities::pending_heatmaps, 42).await.unwrap(),
        vec![fixtures::activity_task()]
    );
    assert_eq!(operations.received.lock().unwrap().len(), 8);
}

#[tokio::test]
async fn batch_handler_storage_failure_is_retryable() {
    use crate::activities::BatchActivities;
    let environment = environment(Arc::new(Operations {
        fail: true,
        ..Default::default()
    }));
    assert_retryable(&environment.run(BatchActivities::prepare_archive, 7).await.unwrap_err());
}
