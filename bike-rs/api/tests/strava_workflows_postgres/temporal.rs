//! Real Temporal execution, isolated PostgreSQL, fake provider, real derived processors.
use super::*;
use bike_core::{
    background_jobs::{entities::pipeline_runs, worker::TaskWorker},
    strava::workflow::{steps::*, Source},
};
use std::time::Duration as Timeout;
use temporalio_client::{
    Client, WorkflowCancelOptions, WorkflowFetchHistoryOptions, WorkflowGetResultOptions,
    WorkflowIdReusePolicy, WorkflowStartOptions,
};
use temporalio_common::protos::temporal::api::history::v1::{
    history_event::Attributes, HistoryEvent,
};
use temporalio_sdk::Runtime;
use worker::temporal::{self, StravaActivityDelete, StravaActivityUpsert, TemporalConfig};

const ACTIVITY_ID: i64 = 99133701;

pub(super) async fn verify(fixture: &Fixture) {
    tokio::task::LocalSet::new()
        .run_until(verify_execution(fixture))
        .await;
}

async fn verify_execution(fixture: &Fixture) {
    let config = TemporalConfig {
        address: std::env::var("BIKE_TEMPORAL_TEST_ADDRESS").unwrap(),
        namespace: "bike".into(),
        task_queue: format!("bike-strava-test-{}", uuid::Uuid::new_v4()),
    };
    let client = temporal::connect(&config).await.unwrap();
    let runtime = Runtime::from_current_tokio(Default::default()).unwrap();
    let mut worker =
        temporal::worker(&runtime, client.clone(), &config, fixture.db.clone()).unwrap();
    let shutdown = worker.shutdown_handle();
    let running = tokio::task::spawn_local(async move { worker.run().await.unwrap() });

    let (activity_id, derived_worker) = first_upsert(fixture, &client, &config).await;
    update_and_retry(fixture, &client, &config, activity_id).await;
    delete_and_reject_stale(fixture, &client, &config).await;
    cancellation(fixture, &client, &config).await;
    derived_worker.abort();
    assert!(derived_worker.await.unwrap_err().is_cancelled());
    failed_child(fixture, &client, &config).await;
    shutdown();
    tokio::time::timeout(Timeout::from_secs(15), running)
        .await
        .unwrap()
        .unwrap();
}

async fn first_upsert(
    fixture: &Fixture,
    client: &Client,
    config: &TemporalConfig,
) -> (i32, tokio::task::JoinHandle<()>) {
    let first = receipt(fixture, "upsert", 100, ACTIVITY_ID).await;
    let handle = client
        .start_workflow(
            StravaActivityUpsert::run,
            first.clone(),
            options(config, &first),
        )
        .await
        .unwrap();
    wait_for_publication_step(client, &first).await;
    let state = pipeline_runs::Model::publication_state(&fixture.db, &first.workflow_id)
        .await
        .unwrap();
    assert!(!state.ready(), "Completion must wait for real derived work");
    assert_ne!(
        strava_workflows::Model::load(&fixture.db, &first.workflow_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "completed"
    );
    let derived_worker = derived_worker(fixture);
    let result = tokio::time::timeout(
        Timeout::from_secs(90),
        handle.get_result(WorkflowGetResultOptions::default()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.outcome, "applied");
    assert!(result.activity_id.is_some() && result.import_id.is_some());
    assert!(
        pipeline_runs::Model::publication_state(&fixture.db, &first.workflow_id)
            .await
            .unwrap()
            .ready()
    );
    verify_history(
        client,
        &first,
        &[
            "strava_fetch_activity",
            "strava_fetch_streams",
            "strava_apply_activity",
            "strava_enqueue_derived_work",
            "strava_wait_for_publication",
            "strava_complete_operation",
        ],
    )
    .await;
    assert!(client
        .start_workflow(
            StravaActivityUpsert::run,
            first.clone(),
            options(config, &first)
        )
        .await
        .is_err());
    assert_eq!(fixture.vendor.detail_calls.load(Ordering::SeqCst), 1);

    (result.activity_id.unwrap(), derived_worker)
}

async fn receipt(fixture: &Fixture, action: &str, offset: i64, activity_id: i64) -> Operation {
    let id = uuid::Uuid::new_v4().to_string();
    let source = Source {
        generation: fixture.connection.created_at,
        event_time: fixture.base_time + offset,
        parent: None,
        activity: None,
        listing_page: None,
        raw: None,
        after: None,
        revoke_remote: false,
    };
    strava_workflows::ActiveModel {
        id: Set(id.clone()),
        connection_id: Set(fixture.connection.id),
        athlete_id: Set(fixture.connection.athlete_id),
        activity_id: Set(Some(activity_id)),
        action: Set(action.into()),
        source: Set(serde_json::to_value(source).unwrap()),
        checkpoint: Set(json!({})),
        status: Set("accepted".into()),
        error: Set(None),
        accepted_at: Set(Utc::now()),
        completed_at: Set(None),
    }
    .insert(&fixture.db)
    .await
    .unwrap();
    Operation { workflow_id: id }
}

fn options(config: &TemporalConfig, operation: &Operation) -> WorkflowStartOptions {
    WorkflowStartOptions::new(config.task_queue.clone(), operation.workflow_id.clone())
        .id_reuse_policy(WorkflowIdReusePolicy::RejectDuplicate)
        .build()
}

fn derived_worker(fixture: &Fixture) -> tokio::task::JoinHandle<()> {
    use worker::tasks::{
        PrepareHeatmap, RebuildFitnessFreshness, RebuildSegmentAnalytics, RegenerateSegmentEfforts,
    };
    let db = fixture.db.clone();
    let worker = TaskWorker::new(db.clone())
        .register_processor(Arc::new(RebuildFitnessFreshness::new(db.clone())))
        .register_processor(Arc::new(RebuildSegmentAnalytics::new(db.clone())))
        .register_processor(Arc::new(RegenerateSegmentEfforts::new(db.clone())))
        .register_processor(Arc::new(PrepareHeatmap::new(db)))
        .with_poll_interval(Timeout::from_secs(1));
    tokio::spawn(worker.run())
}

async fn history(client: &Client, operation: &Operation) -> Vec<HistoryEvent> {
    client
        .get_workflow_handle::<StravaActivityUpsert>(&operation.workflow_id)
        .fetch_history(WorkflowFetchHistoryOptions::default())
        .into_events()
        .await
        .unwrap()
}

async fn wait_for_publication_step(client: &Client, operation: &Operation) {
    tokio::time::timeout(Timeout::from_secs(60), async {
        loop {
            if history(client, operation).await.iter().any(|event| matches!(&event.attributes,
                Some(Attributes::ActivityTaskScheduledEventAttributes(step)) if step.activity_type.as_ref().unwrap().name == "strava_wait_for_publication")) { break; }
            tokio::time::sleep(Timeout::from_millis(100)).await;
        }
    }).await.unwrap();
}

async fn verify_history(client: &Client, operation: &Operation, expected: &[&str]) {
    let events = history(client, operation).await;
    let mut steps = Vec::new();
    let mut completions = 0;
    for event in &events {
        match &event.attributes {
            Some(Attributes::ActivityTaskScheduledEventAttributes(step)) => {
                steps.push(step.activity_type.as_ref().unwrap().name.as_str());
                let input: Value =
                    serde_json::from_slice(&step.input.as_ref().unwrap().payloads[0].data).unwrap();
                assert!(
                    input["workflow_id"] == operation.workflow_id
                        || input["mutation"]["workflow_id"] == operation.workflow_id
                );
            }
            Some(Attributes::ActivityTaskCompletedEventAttributes(step)) => {
                assert!(!step.result.as_ref().unwrap().payloads[0].data.is_empty());
                completions += 1;
            }
            _ => {}
        }
    }
    assert_eq!(steps, expected);
    assert_eq!(completions, expected.len());
}

async fn upsert(
    fixture: &Fixture,
    client: &Client,
    config: &TemporalConfig,
    offset: i64,
) -> OperationResult {
    let operation = receipt(fixture, "upsert", offset, ACTIVITY_ID).await;
    let handle = client
        .start_workflow(
            StravaActivityUpsert::run,
            operation.clone(),
            options(config, &operation),
        )
        .await
        .unwrap();
    tokio::time::timeout(
        Timeout::from_secs(90),
        handle.get_result(WorkflowGetResultOptions::default()),
    )
    .await
    .unwrap()
    .unwrap()
}

async fn update_and_retry(
    fixture: &Fixture,
    client: &Client,
    config: &TemporalConfig,
    activity_id: i32,
) {
    let original = fixture.vendor.activity.lock().unwrap()["name"].clone();
    fixture.vendor.activity.lock().unwrap()["name"] = json!("Temporal updated ride");
    let updated = upsert(fixture, client, config, 110).await;
    assert_eq!(updated.activity_id, Some(activity_id));
    assert_eq!(
        activities::Entity::find_by_id(activity_id)
            .one(&fixture.db)
            .await
            .unwrap()
            .unwrap()
            .title,
        "Temporal updated ride"
    );
    let unchanged = upsert(fixture, client, config, 120).await;
    assert_eq!(unchanged.outcome, "unchanged");
    assert_eq!(unchanged.import_id, updated.import_id);

    let before = fixture.vendor.detail_calls.load(Ordering::SeqCst);
    fixture.vendor.detail_status.store(500, Ordering::SeqCst);
    let operation = receipt(fixture, "upsert", 130, ACTIVITY_ID).await;
    let handle = client
        .start_workflow(
            StravaActivityUpsert::run,
            operation.clone(),
            options(config, &operation),
        )
        .await
        .unwrap();
    wait_for_provider_call(fixture, before).await;
    fixture.vendor.detail_status.store(0, Ordering::SeqCst);
    let retried = tokio::time::timeout(
        Timeout::from_secs(90),
        handle.get_result(WorkflowGetResultOptions::default()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(retried.outcome, "unchanged");
    assert_eq!(retried.activity_id, Some(activity_id));
    assert!(fixture.vendor.detail_calls.load(Ordering::SeqCst) >= before + 2);
    fixture.vendor.activity.lock().unwrap()["name"] = original;
}

async fn delete_and_reject_stale(fixture: &Fixture, client: &Client, config: &TemporalConfig) {
    let reads = fixture.vendor.detail_calls.load(Ordering::SeqCst);
    let operation = receipt(fixture, "delete", 150, ACTIVITY_ID).await;
    let handle = client
        .start_workflow(
            StravaActivityDelete::run,
            operation.clone(),
            options(config, &operation),
        )
        .await
        .unwrap();
    let result = tokio::time::timeout(
        Timeout::from_secs(90),
        handle.get_result(WorkflowGetResultOptions::default()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.outcome, "deleted");
    assert!(activities::Model::strava_activity(
        &fixture.db,
        fixture.connection.user_id,
        ACTIVITY_ID
    )
    .await
    .unwrap()
    .is_none());
    verify_history(
        client,
        &operation,
        &[
            "strava_capture_deletion",
            "strava_delete_activity",
            "strava_enqueue_derived_work",
            "strava_wait_for_publication",
            "strava_complete_operation",
        ],
    )
    .await;
    assert_eq!(
        upsert(fixture, client, config, 140).await.outcome,
        "superseded"
    );
    assert_eq!(fixture.vendor.detail_calls.load(Ordering::SeqCst), reads);
}

async fn wait_for_provider_call(fixture: &Fixture, before: usize) {
    tokio::time::timeout(Timeout::from_secs(30), async {
        while fixture.vendor.detail_calls.load(Ordering::SeqCst) == before {
            tokio::time::sleep(Timeout::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
}

async fn cancellation(fixture: &Fixture, client: &Client, config: &TemporalConfig) {
    let before = fixture.vendor.detail_calls.load(Ordering::SeqCst);
    fixture.vendor.detail_status.store(500, Ordering::SeqCst);
    let operation = receipt(fixture, "upsert", 160, ACTIVITY_ID + 1).await;
    let handle = client
        .start_workflow(
            StravaActivityUpsert::run,
            operation.clone(),
            options(config, &operation),
        )
        .await
        .unwrap();
    wait_for_provider_call(fixture, before).await;
    handle
        .cancel(WorkflowCancelOptions::default())
        .await
        .unwrap();
    assert!(tokio::time::timeout(
        Timeout::from_secs(15),
        handle.get_result(WorkflowGetResultOptions::default())
    )
    .await
    .unwrap()
    .is_err());
    let events = history(client, &operation).await;
    assert!(events.iter().any(|event| matches!(
        event.attributes,
        Some(Attributes::WorkflowExecutionCanceledEventAttributes(_))
    )));
    assert!(!events.iter().any(|event| matches!(&event.attributes,
        Some(Attributes::ActivityTaskScheduledEventAttributes(step)) if step.activity_type.as_ref().unwrap().name == "strava_apply_activity")));
    assert!(activities::Model::strava_activity(
        &fixture.db,
        fixture.connection.user_id,
        ACTIVITY_ID + 1
    )
    .await
    .unwrap()
    .is_none());
}

async fn failed_child(fixture: &Fixture, client: &Client, config: &TemporalConfig) {
    use bike_core::background_jobs::entities::pipeline_tasks;
    fixture.vendor.detail_status.store(0, Ordering::SeqCst);
    let operation = receipt(fixture, "upsert", 170, ACTIVITY_ID + 2).await;
    let handle = client
        .start_workflow(
            StravaActivityUpsert::run,
            operation.clone(),
            options(config, &operation),
        )
        .await
        .unwrap();
    wait_for_publication_step(client, &operation).await;
    let linked = pipeline_tasks::Entity::find()
        .filter(pipeline_tasks::Column::RunId.eq(&operation.workflow_id))
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    let task = background_tasks::Entity::find_by_id(linked.task_id)
        .one(&fixture.db)
        .await
        .unwrap()
        .unwrap();
    let mut task: background_tasks::ActiveModel = task.into();
    task.status = Set("failed".into());
    task.update(&fixture.db).await.unwrap();
    assert!(tokio::time::timeout(
        Timeout::from_secs(15),
        handle.get_result(WorkflowGetResultOptions::default())
    )
    .await
    .unwrap()
    .is_err());
    assert_ne!(
        strava_workflows::Model::load(&fixture.db, &operation.workflow_id)
            .await
            .unwrap()
            .unwrap()
            .status,
        "completed"
    );
    assert!(history(client, &operation)
        .await
        .iter()
        .any(|event| matches!(
            event.attributes,
            Some(Attributes::WorkflowExecutionFailedEventAttributes(_))
        )));
}
