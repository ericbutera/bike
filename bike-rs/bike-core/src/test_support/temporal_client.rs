use prost::Message;
use std::sync::{Arc, Mutex};
use temporalio_client::{
    callback_based::{CallbackBasedGrpcService, GrpcSuccessResponse},
    Client, ClientOptions, Connection, ConnectionOptions,
};
use temporalio_common::protos::temporal::api::workflowservice::v1::{
    CreateScheduleRequest, CreateScheduleResponse, StartWorkflowExecutionRequest, StartWorkflowExecutionResponse,
};

#[derive(Default)]
pub struct Requests {
    pub starts: Vec<StartWorkflowExecutionRequest>,
    pub fail_start: bool,
    pub already_started: bool,
    pub schedules: Vec<CreateScheduleRequest>,
    pub schedule_exists: bool,
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
                    "CreateSchedule" => {
                        let schedule = CreateScheduleRequest::decode(request.proto).unwrap();
                        let mut requests = requests.lock().unwrap();
                        requests.schedules.push(schedule);
                        if requests.schedule_exists {
                            return Err(tonic::Status::already_exists("existing schedule"));
                        }
                        CreateScheduleResponse::default().encode_to_vec()
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

fn already_started() -> tonic::Status {
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
