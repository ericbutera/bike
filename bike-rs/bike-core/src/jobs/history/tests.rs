use super::*;
use crate::test_support::temporal_client::{self, Requests};
use sea_orm::{DatabaseBackend, MockDatabase};
use serde_json::json;
use std::sync::{Arc, Mutex};
use temporalio_common::protos::temporal::api::{
    common::v1::WorkflowType, workflowservice::v1::ListWorkflowExecutionsResponse,
};

async fn queue(requests: Arc<Mutex<Requests>>) -> JobQueue {
    JobQueue::with_client(
        MockDatabase::new(DatabaseBackend::Postgres).into_connection(),
        temporal_client::client(requests).await,
    )
}

#[tokio::test]
async fn lists_native_statuses_and_pages_without_loading_every_input() {
    let requests = Arc::new(Mutex::new(Requests {
        list_response: ListWorkflowExecutionsResponse {
            executions: (1..=7)
                .map(|status| WorkflowExecutionInfo {
                    execution: Some(WorkflowExecution {
                        workflow_id: "fitness".into(),
                        run_id: "run".into(),
                    }),
                    r#type: Some(WorkflowType {
                        name: "rebuild_fitness_freshness".into(),
                    }),
                    status,
                    start_time: Some(Default::default()),
                    close_time: Some(Default::default()),
                    ..Default::default()
                })
                .collect(),
            next_page_token: vec![1, 2],
        },
        ..Default::default()
    }));
    let page = queue(requests.clone())
        .await
        .execution_history(Some(TaskStatus::Completed), vec![3])
        .await
        .unwrap();
    assert_eq!(page.next_page_token.as_deref(), Some("0102"));
    let values = serde_json::to_value(&page).unwrap();
    let statuses: Vec<_> = values["executions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| item["status"].as_str().unwrap())
        .collect();
    assert_eq!(
        statuses,
        [
            "Running",
            "Completed",
            "Failed",
            "Canceled",
            "Terminated",
            "ContinuedAsNew",
            "TimedOut"
        ]
    );
    let first = &page.executions[0];
    assert_eq!(first.job.namespace, "bike");
    assert_eq!(first.job.workflow_type, "rebuild_fitness_freshness");
    assert_eq!(first.started_at.unwrap().timestamp(), 0);
    assert_eq!(first.closed_at.unwrap().timestamp(), 0);
    let requests = requests.lock().unwrap();
    assert!(requests.histories.is_empty());
    assert_eq!(requests.lists[0].namespace, "bike");
    assert_eq!(requests.lists[0].page_size, 25);
    assert_eq!(requests.lists[0].next_page_token, vec![3]);
    assert_eq!(
        requests.lists[0].query,
        "TaskQueue IN ('bike-tasks', 'bike-strava') AND ExecutionStatus = 'Completed'"
    );
}

#[tokio::test]
async fn reads_original_parameters_for_the_selected_run_only() {
    let parameters = json!({"user_id": 7, "after_id": 12});
    let requests = Arc::new(Mutex::new(Requests {
        history_response: temporal_client::execution_input(parameters.clone()),
        ..Default::default()
    }));
    let input = queue(requests.clone())
        .await
        .execution_input("workflow".into(), "specific-run".into())
        .await
        .unwrap();
    assert_eq!(input.parameters, vec![parameters]);
    let requests = requests.lock().unwrap();
    let request = &requests.histories[0];
    assert_eq!(request.maximum_page_size, 1);
    assert!(!request.wait_new_event);
    assert_eq!(request.execution.as_ref().unwrap().run_id, "specific-run");
}

#[tokio::test]
async fn empty_history_and_unavailable_temporal_do_not_claim_success() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let queue = queue(requests.clone()).await;
    let page = queue.execution_history(None, vec![]).await.unwrap();
    assert!(page.executions.is_empty());
    assert!(page.next_page_token.is_none());
    assert!(queue.execution_input("workflow".into(), "run".into()).await.is_err());
    requests.lock().unwrap().fail_reads = true;
    assert!(queue.execution_history(None, vec![]).await.is_err());
}

#[tokio::test]
async fn refuses_inputs_from_another_queue() {
    let mut response = temporal_client::execution_input(json!({"user_id": 7}));
    let event = &mut response.history.as_mut().unwrap().events[0];
    let Some(Attributes::WorkflowExecutionStartedEventAttributes(started)) = &mut event.attributes else {
        panic!()
    };
    started.task_queue.as_mut().unwrap().name = "another-app".into();
    let requests = Arc::new(Mutex::new(Requests {
        history_response: response,
        ..Default::default()
    }));
    assert!(queue(requests)
        .await
        .execution_input("workflow".into(), "run".into())
        .await
        .is_err());
}
