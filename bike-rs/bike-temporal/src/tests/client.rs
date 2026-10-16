use super::*;
use futures::FutureExt;
use prost::Message;
use std::sync::{Arc, Mutex};
use temporalio_client::{
    callback_based::{CallbackBasedGrpcService, GrpcSuccessResponse},
    Client, ClientOptions, Connection, ConnectionOptions,
};
use temporalio_common::protos::temporal::api::{
    enums::v1::{WorkflowIdConflictPolicy, WorkflowIdReusePolicy},
    workflowservice::v1::{
        SignalWithStartWorkflowExecutionRequest, SignalWithStartWorkflowExecutionResponse,
        StartWorkflowExecutionRequest, StartWorkflowExecutionResponse,
    },
};

#[derive(Default)]
pub(super) struct Requests {
    pub signals: Vec<SignalWithStartWorkflowExecutionRequest>,
    pub starts: Vec<StartWorkflowExecutionRequest>,
}

pub(super) async fn client(requests: Arc<Mutex<Requests>>, fail: bool) -> Client {
    let mut options = ConnectionOptions::new(
        "http://fixture.invalid:7233"
            .parse::<temporalio_client::Url>()
            .unwrap(),
    )
    .build();
    options.dns_load_balancing = None;
    options.service_override = Some(CallbackBasedGrpcService {
        callback: Arc::new(move |request| {
            let requests = requests.clone();
            async move {
                let proto = match request.rpc.as_str() {
                    "GetSystemInfo" => vec![],
                    "SignalWithStartWorkflowExecution" => {
                        requests.lock().unwrap().signals.push(
                            SignalWithStartWorkflowExecutionRequest::decode(request.proto).unwrap(),
                        );
                        if fail {
                            return Err(tonic::Status::permission_denied("fixture refused"));
                        }
                        SignalWithStartWorkflowExecutionResponse {
                            run_id: "fixture-run".into(),
                            ..Default::default()
                        }
                        .encode_to_vec()
                    }
                    "StartWorkflowExecution" => {
                        requests
                            .lock()
                            .unwrap()
                            .starts
                            .push(StartWorkflowExecutionRequest::decode(request.proto).unwrap());
                        StartWorkflowExecutionResponse {
                            run_id: "fixture-run".into(),
                            ..Default::default()
                        }
                        .encode_to_vec()
                    }
                    other => panic!("Unexpected RPC {other}"),
                };
                Ok(GrpcSuccessResponse {
                    headers: Default::default(),
                    proto,
                })
            }
            .boxed()
        }),
    });
    Client::new(
        Connection::connect(options).await.unwrap(),
        ClientOptions::new("bike").build(),
    )
    .unwrap()
}

#[tokio::test]
async fn repeated_parent_handoffs_use_native_atomic_start_for_the_same_named_outputs() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let client = client(requests.clone(), false).await;
    for _ in 0..5 {
        crate::client::request_fitness_freshness(
            &client,
            fitness_change(),
            DebouncePolicy::default(),
        )
        .await
        .unwrap();
        crate::client::request_segment_analytics(
            &client,
            segment_change(),
            DebouncePolicy::default(),
        )
        .await
        .unwrap();
        crate::client::request_heatmap(&client, heatmap_change(), DebouncePolicy::default())
            .await
            .unwrap();
    }
    let requests = requests.lock().unwrap();
    assert_eq!(requests.signals.len(), 15);
    let expected = [
        (
            "rebuild_fitness_freshness",
            crate::client::fitness_workflow_id(7),
        ),
        (
            "rebuild_segment_analytics",
            crate::client::segment_workflow_id(18),
        ),
        ("prepare_heatmap", crate::client::heatmap_workflow_id(42)),
    ];
    for (request, (name, id)) in requests.signals.iter().zip(expected.iter().cycle()) {
        assert_eq!(request.namespace, "bike");
        assert_eq!(request.workflow_id, *id);
        assert_eq!(request.workflow_type.as_ref().unwrap().name, *name);
        assert_eq!(request.task_queue.as_ref().unwrap().name, crate::TASK_QUEUE);
        assert_eq!(
            request.workflow_id_conflict_policy,
            WorkflowIdConflictPolicy::UseExisting as i32
        );
        assert_eq!(
            request.workflow_id_reuse_policy,
            WorkflowIdReusePolicy::AllowDuplicateFailedOnly as i32
        );
        assert_eq!(request.signal_name, "changed");
        assert_eq!(request.priority.as_ref().unwrap().priority_key, 5);
    }
}

#[tokio::test]
async fn zero_revision_is_rejected_before_rpc_and_native_rejection_is_returned() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let client = client(requests.clone(), true).await;
    assert!(crate::client::request_fitness_freshness(
        &client,
        change(0, Priority::Background),
        DebouncePolicy::default()
    )
    .await
    .is_err());
    assert!(requests.lock().unwrap().signals.is_empty());
    assert!(crate::client::request_fitness_freshness(
        &client,
        fitness_change(),
        DebouncePolicy::default()
    )
    .await
    .is_err());
    assert_eq!(requests.lock().unwrap().signals.len(), 1);
}

#[tokio::test]
async fn import_entry_points_preserve_task_names_and_retry_stable_command_ids() {
    let requests = Arc::new(Mutex::new(Requests::default()));
    let client = client(requests.clone(), false).await;
    let source = import_command().source;
    crate::client::process_activity_import(&client, source.clone())
        .await
        .unwrap();
    let input = ReprocessInput {
        source,
        activity_id: 42,
        expected_revision: 3,
    };
    crate::client::reprocess_activity_import(&client, input.clone())
        .await
        .unwrap();
    crate::client::reprocess_activity_import(&client, input)
        .await
        .unwrap();
    let requests = requests.lock().unwrap();
    let starts = &requests.starts;
    assert_eq!(starts.len(), 3);
    assert_eq!(
        starts[0].workflow_type.as_ref().unwrap().name,
        "process_activity_import"
    );
    assert_eq!(
        starts[1].workflow_type.as_ref().unwrap().name,
        "reprocess_activity_import"
    );
    assert_eq!(starts[1].workflow_id, starts[2].workflow_id);
    assert_eq!(starts[1].priority.as_ref().unwrap().priority_key, 1);
}
