use super::fixtures::{environment, Operations};
use super::*;
use crate::{
    activities::{DerivedActivities, ImportActivities},
    scheduling::SchedulingActivities,
};
use bike_core::bike_tasks::SegmentChange;
use std::sync::{Arc, Mutex};
use temporalio_sdk::testing::ActivityEnvironment;

#[tokio::test]
async fn each_named_activity_calls_its_domain_operation_and_preserves_failure() {
    for fail in [false, true] {
        let operations = Arc::new(Operations {
            fail,
            ..Default::default()
        });
        let environment = environment(operations.clone());
        let imported = environment
            .run(ImportActivities::activity_import, import_command())
            .await;
        assert_eq!(imported.is_err(), fail);
        if !fail {
            assert_eq!(imported.unwrap(), import_result());
        }
        assert_eq!(
            environment
                .run(DerivedActivities::rebuild_fitness_freshness, fitness_change())
                .await
                .is_err(),
            fail
        );
        assert_eq!(
            environment
                .run(DerivedActivities::rebuild_segment_analytics, segment_change())
                .await
                .is_err(),
            fail
        );
        assert_eq!(
            environment
                .run(DerivedActivities::prepare_heatmap, heatmap_change())
                .await
                .is_err(),
            fail
        );
        assert_eq!(
            *operations.received.lock().unwrap(),
            vec![
                ("activity_import", serde_json::to_value(import_command()).unwrap()),
                (
                    "rebuild_fitness_freshness",
                    serde_json::to_value(fitness_change()).unwrap()
                ),
                (
                    "rebuild_segment_analytics",
                    serde_json::to_value(segment_change()).unwrap()
                ),
                ("prepare_heatmap", serde_json::to_value(heatmap_change()).unwrap()),
            ]
        );
    }
}

#[tokio::test]
async fn invalid_import_command_never_calls_the_domain_adapter() {
    let operations = Arc::new(Operations::default());
    let environment = environment(operations.clone());
    let mut command = import_command();
    command.source.command_id.clear();
    assert!(environment
        .run(ImportActivities::activity_import, command)
        .await
        .is_err());
    assert!(operations.received.lock().unwrap().is_empty());
}

#[tokio::test]
async fn shared_import_activity_preserves_new_activity_and_background_refresh_priorities() {
    let operations = Arc::new(Operations::default());
    let environment = environment(operations);
    for priority in [Priority::NewActivity, Priority::Background] {
        let mut command = import_command();
        command.source.priority = priority;
        command.target = bike_core::bike_tasks::ImportTarget::ExistingActivity {
            activity_id: 42,
            expected_revision: 3,
        };
        let result = environment
            .run(ImportActivities::activity_import, command)
            .await
            .unwrap();
        assert_eq!(result, import_result().with_priority(priority));
    }
}

#[tokio::test]
async fn bulk_new_imports_keep_background_refresh_priority() {
    let environment = environment(Arc::new(Operations::default()));
    let mut command = import_command();
    command.source.priority = Priority::Background;
    let result = environment
        .run(ImportActivities::activity_import, command)
        .await
        .unwrap();
    assert_eq!(result, import_result().with_priority(Priority::Background));
}

#[tokio::test]
async fn scheduling_activities_handoff_each_output_and_allow_retried_fanout() {
    let requests = Arc::new(Mutex::new(client::Requests::default()));
    let client = client::client(requests.clone(), false).await;
    let environment = ActivityEnvironment::builder_with_default()
        .register_activities(SchedulingActivities::new(client, DebouncePolicy::default()))
        .build();
    environment
        .run(SchedulingActivities::schedule_fitness_freshness, fitness_change())
        .await
        .unwrap();
    for _ in 0..2 {
        environment
            .run(SchedulingActivities::schedule_segment_analytics, vec![segment_change()])
            .await
            .unwrap();
    }
    environment
        .run(SchedulingActivities::schedule_heatmap, heatmap_change())
        .await
        .unwrap();
    assert_eq!(requests.lock().unwrap().signals.len(), 4);
    environment
        .run(SchedulingActivities::schedule_segment_analytics, vec![])
        .await
        .unwrap();
    assert_eq!(requests.lock().unwrap().signals.len(), 4);
}

#[tokio::test]
async fn scheduling_validates_segment_fanout_before_partial_submission() {
    let requests = Arc::new(Mutex::new(client::Requests::default()));
    let client = client::client(requests.clone(), false).await;
    let environment = ActivityEnvironment::builder_with_default()
        .register_activities(SchedulingActivities::new(client, DebouncePolicy::default()))
        .build();
    let invalid = SegmentChange {
        revision: 0,
        ..segment_change()
    };
    assert!(environment
        .run(
            SchedulingActivities::schedule_segment_analytics,
            vec![segment_change(), invalid]
        )
        .await
        .is_err());
    assert!(requests.lock().unwrap().signals.is_empty());
}

#[tokio::test]
async fn rejected_native_handoff_is_an_activity_failure() {
    let requests = Arc::new(Mutex::new(client::Requests::default()));
    let client = client::client(requests, true).await;
    let environment = ActivityEnvironment::builder_with_default()
        .register_activities(SchedulingActivities::new(client, DebouncePolicy::default()))
        .build();
    assert!(environment
        .run(SchedulingActivities::schedule_fitness_freshness, fitness_change())
        .await
        .is_err());
}
