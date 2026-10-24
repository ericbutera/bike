use bike_core::{bike_tasks::ReprocessInput, test_support as fixtures};
pub(crate) use fixtures::temporal_client::{client, Requests};
use std::sync::{Arc, Mutex};

pub(crate) struct SubmissionFixture {
    pub service: crate::submission::TaskSubmissionService,
    pub requests: Arc<Mutex<Requests>>,
}

impl SubmissionFixture {
    pub async fn new() -> Self {
        let requests = Arc::new(Mutex::new(Requests::default()));
        let service = crate::submission::TaskSubmissionService::new(client(requests.clone()).await);
        Self { service, requests }
    }

    pub async fn process_activity_import(&self) -> Result<bike_core::bike_tasks::JobReference, crate::client::Error> {
        self.service
            .process_activity_import(fixtures::import_command().source, fixtures::task_cause())
            .await
    }

    pub async fn reprocess_activity_import(&self) -> Result<bike_core::bike_tasks::JobReference, crate::client::Error> {
        self.service
            .reprocess_activity_import(fixtures::reprocess_input(), fixtures::task_cause())
            .await
    }
}

#[tokio::test]
async fn repeated_submission_returns_the_existing_job_without_history_reads() {
    let fixture = SubmissionFixture::new().await;
    let first = fixture.reprocess_activity_import().await.unwrap();
    let retry = fixture.reprocess_activity_import().await.unwrap();
    assert_eq!(first, retry);
    let requests = fixture.requests.lock().unwrap();
    assert_eq!(requests.starts.len(), 2);
    let start = &requests.starts[0];
    assert_eq!(start.workflow_type.as_ref().unwrap().name, "reprocess_activity_import");
    assert_eq!(start.task_queue.as_ref().unwrap().name, crate::TASK_QUEUE);
    assert!(start.retry_policy.is_none());
    assert_eq!(start.priority.as_ref().unwrap().priority_key, 1);
    assert_eq!(
        serde_json::from_slice::<String>(&start.search_attributes.as_ref().unwrap().indexed_fields["BikeUserId"].data)
            .unwrap(),
        fixtures::customer().id.to_string()
    );
}

#[tokio::test]
async fn the_same_command_for_a_different_activity_starts_its_own_job() {
    let fixture = SubmissionFixture::new().await;
    let first = fixture.reprocess_activity_import().await.unwrap();
    let other = fixture
        .service
        .reprocess_activity_import(
            ReprocessInput {
                activity_id: 99,
                ..fixtures::reprocess_input()
            },
            fixtures::task_cause(),
        )
        .await
        .unwrap();
    assert_ne!(first.job_id, other.job_id);
}

#[tokio::test]
async fn domain_producers_start_native_workflows_without_sql_queue_writes() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let db = fixtures::postgres_mock().into_connection();
    let queue = bike_core::jobs::JobQueue::with_client(db.clone(), client(requests.clone()).await);
    queue.rebuild_fitness_freshness(12).await.unwrap();
    queue.rebuild_segment_analytics(vec![18, 18, 0, -1, 19]).await.unwrap();
    queue.regenerate_segment_efforts(18).await.unwrap();
    queue.regenerate_user_segments(12).await.unwrap();
    queue.reprocess_user_activity_imports(12).await.unwrap();
    queue.reprocess_archive_fit_activity_imports(12).await.unwrap();
    queue.backfill_user_xc_training(12).await.unwrap();
    queue.archive_activity_import(7, 12).await.unwrap();
    queue.rebuild_segment_analytics(vec![0, -1]).await.unwrap();
    let starts = &requests.lock().unwrap().starts;
    let names: Vec<_> = starts
        .iter()
        .map(|request| request.workflow_type.as_ref().unwrap().name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "rebuild_fitness_freshness",
            "rebuild_segment_analytics",
            "regenerate_segment_efforts",
            "regenerate_user_segments",
            "reprocess_user_activity_imports",
            "reprocess_archive_fit_activity_imports",
            "backfill_user_xc_training",
            "activity_archive_import"
        ]
    );
    let input: serde_json::Value = serde_json::from_slice(&starts[1].input.as_ref().unwrap().payloads[0].data).unwrap();
    assert_eq!(input["segment_ids"], serde_json::json!([18, 19]));
    assert_eq!(starts.len(), 8);
    for start in starts {
        assert_eq!(start.task_queue.as_ref().unwrap().name, crate::TASK_QUEUE);
        assert!(start.retry_policy.is_none());
    }
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn direct_producer_reports_a_rejected_native_start() {
    let requests = Arc::new(Mutex::new(Requests {
        fail_start: true,
        ..Default::default()
    }));
    let queue =
        bike_core::jobs::JobQueue::with_client(fixtures::postgres_mock().into_connection(), client(requests).await);
    assert!(queue
        .archive_activity_import(7, 12)
        .await
        .unwrap_err()
        .contains("fixture start refused"));
}

#[tokio::test]
async fn training_submission_records_queued_state_before_start_and_reports_rejection() {
    for fail_start in [false, true] {
        let requests = Arc::new(Mutex::new(Requests {
            fail_start,
            ..Default::default()
        }));
        let mut mock = fixtures::postgres_mock()
            .append_query_results([vec![fixtures::user_preferences()], vec![fixtures::user_preferences()]]);
        if fail_start {
            mock = mock.append_query_results([vec![fixtures::user_preferences()], vec![fixtures::user_preferences()]]);
        }
        let db = mock.into_connection();
        let queue = bike_core::jobs::JobQueue::with_client(db.clone(), client(requests.clone()).await);
        let result =
            bike_core::xc_goal_backfill::queue_user_xc_goal_backfill(&db, &queue, fixtures::customer().id).await;
        assert_eq!(result.is_err(), fail_start);
        assert_eq!(requests.lock().unwrap().starts.len(), 1);
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("queued"));
        assert_eq!(log.contains("failed"), fail_start);
        assert!(!log.contains("activity_import_locks"));
    }
}

#[tokio::test]
async fn retained_file_producers_submit_owned_metadata_without_processing_in_the_api() {
    for reprocessing in [false, true] {
        let requests = Arc::new(Mutex::new(Requests::default()));
        let mut mock = fixtures::postgres_mock();
        if reprocessing {
            mock = mock.append_query_results([vec![fixtures::reprocess_activity()]]);
        }
        let db = mock
            .append_query_results([vec![fixtures::reprocess_import()]])
            .append_query_results([vec![fixtures::reprocess_artifact()]])
            .into_connection();
        let queue = bike_core::jobs::JobQueue::with_client(db.clone(), client(requests.clone()).await);
        if reprocessing {
            queue.reprocess_activity_import(42).await.unwrap();
        } else {
            queue.process_activity_import(7, 18).await.unwrap();
        }
        let requests = requests.lock().unwrap();
        let request = &requests.starts[0];
        let payload: serde_json::Value =
            serde_json::from_slice(&request.input.as_ref().unwrap().payloads[0].data).unwrap();
        let source = if reprocessing { &payload["source"] } else { &payload };
        assert_eq!(source["user_id"], 7);
        assert_eq!(source["activity_import_id"], 18);
        assert_eq!(source["object"]["path"], fixtures::reprocess_artifact().storage_path);
        let name = if reprocessing {
            "reprocess_activity_import"
        } else {
            "process_activity_import"
        };
        assert_eq!(request.workflow_type.as_ref().unwrap().name, name);
        assert!(!format!("{:?}", db.into_transaction_log()).contains("INSERT INTO"));
    }
}

#[tokio::test]
async fn large_segment_refreshes_use_bounded_batches_and_empty_inputs_start_nothing() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let queue = bike_core::jobs::JobQueue::with_client(
        fixtures::postgres_mock().into_connection(),
        client(requests.clone()).await,
    );
    queue.rebuild_segment_analytics(Vec::new()).await.unwrap();
    queue
        .rebuild_segment_analytics((1..=501).chain([1, 0, -1]).collect())
        .await
        .unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.starts.len(), 2);
    let inputs: Vec<serde_json::Value> = requests
        .starts
        .iter()
        .map(|start| serde_json::from_slice(&start.input.as_ref().unwrap().payloads[0].data).unwrap())
        .collect();
    assert_eq!(inputs[0]["segment_ids"].as_array().unwrap().len(), 500);
    assert_eq!(inputs[1]["segment_ids"], serde_json::json!([501]));
}
