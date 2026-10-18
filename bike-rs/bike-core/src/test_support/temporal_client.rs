use prost::Message;
use std::sync::{Arc, Mutex};
use temporalio_client::{
    callback_based::{CallbackBasedGrpcService, GrpcSuccessResponse},
    Client, ClientOptions, Connection, ConnectionOptions,
};
use temporalio_common::protos::temporal::api::workflowservice::v1::{
    GetWorkflowExecutionHistoryRequest, GetWorkflowExecutionHistoryResponse, ListWorkflowExecutionsRequest,
    ListWorkflowExecutionsResponse, StartWorkflowExecutionRequest, StartWorkflowExecutionResponse,
};

#[derive(Default)]
pub struct Requests {
    pub starts: Vec<StartWorkflowExecutionRequest>,
    pub fail_start: bool,
    pub already_started: bool,
    pub lists: Vec<ListWorkflowExecutionsRequest>,
    pub histories: Vec<GetWorkflowExecutionHistoryRequest>,
    pub list_response: ListWorkflowExecutionsResponse,
    pub history_response: GetWorkflowExecutionHistoryResponse,
    pub fail_reads: bool,
}

pub async fn client(requests: Arc<Mutex<Requests>>) -> Client {
    let mut options =
        ConnectionOptions::new("http://fixture.invalid:7233".parse::<temporalio_client::Url>().unwrap()).build();
    options.dns_load_balancing = None;
    options.service_override = Some(CallbackBasedGrpcService {
        callback: Arc::new(move |request| {
            let requests = requests.clone();
            Box::pin(async move {
                let proto = match request.rpc.as_str() {
                    "GetSystemInfo" => vec![],
                    "ListWorkflowExecutions" | "GetWorkflowExecutionHistory" => {
                        read_response(&mut requests.lock().unwrap(), &request.rpc, request.proto.as_ref())?
                    }
                    "StartWorkflowExecution" => {
                        let start = StartWorkflowExecutionRequest::decode(request.proto).unwrap();
                        let mut requests = requests.lock().unwrap();
                        let duplicate = requests
                            .starts
                            .iter()
                            .any(|previous| previous.workflow_id == start.workflow_id);
                        requests.starts.push(start);
                        if requests.fail_start {
                            return Err(tonic::Status::permission_denied("fixture start refused"));
                        }
                        if duplicate || requests.already_started {
                            return Err(already_started());
                        }
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
            })
        }),
    });
    Client::new(
        Connection::connect(options).await.unwrap(),
        ClientOptions::new("bike").build(),
    )
    .unwrap()
}

fn read_response(requests: &mut Requests, rpc: &str, proto: &[u8]) -> Result<Vec<u8>, tonic::Status> {
    if requests.fail_reads {
        return Err(tonic::Status::permission_denied("fixture history refused"));
    }
    if rpc == "ListWorkflowExecutions" {
        requests
            .lists
            .push(ListWorkflowExecutionsRequest::decode(proto).unwrap());
        Ok(requests.list_response.encode_to_vec())
    } else {
        requests
            .histories
            .push(GetWorkflowExecutionHistoryRequest::decode(proto).unwrap());
        Ok(requests.history_response.encode_to_vec())
    }
}

pub fn execution_input(parameters: serde_json::Value) -> GetWorkflowExecutionHistoryResponse {
    use temporalio_common::protos::temporal::api::{
        common::v1::{Payload, Payloads},
        history::v1::{history_event::Attributes, History, HistoryEvent, WorkflowExecutionStartedEventAttributes},
        taskqueue::v1::TaskQueue,
    };
    GetWorkflowExecutionHistoryResponse {
        history: Some(History {
            events: vec![HistoryEvent {
                attributes: Some(Attributes::WorkflowExecutionStartedEventAttributes(
                    WorkflowExecutionStartedEventAttributes {
                        task_queue: Some(TaskQueue {
                            name: "bike-tasks".into(),
                            ..Default::default()
                        }),
                        input: Some(Payloads {
                            payloads: vec![Payload {
                                metadata: [("encoding".into(), b"json/plain".to_vec())].into(),
                                data: serde_json::to_vec(&parameters).unwrap(),
                                ..Default::default()
                            }],
                        }),
                        ..Default::default()
                    },
                )),
                ..Default::default()
            }],
        }),
        ..Default::default()
    }
}

pub fn already_started() -> tonic::Status {
    use temporalio_common::protos::{
        google::rpc::Status, temporal::api::errordetails::v1::WorkflowExecutionAlreadyStartedFailure,
        utilities::pack_any,
    };
    let details = Status {
        code: tonic::Code::AlreadyExists as i32,
        message: "fixture duplicate".into(),
        details: vec![pack_any(
            "type.googleapis.com/temporal.api.errordetails.v1.WorkflowExecutionAlreadyStartedFailure".into(),
            &WorkflowExecutionAlreadyStartedFailure {
                run_id: "fixture-run".into(),
                ..Default::default()
            },
        )
        .unwrap()],
    };
    tonic::Status::with_details(
        tonic::Code::AlreadyExists,
        "fixture duplicate",
        details.encode_to_vec().into(),
    )
}
