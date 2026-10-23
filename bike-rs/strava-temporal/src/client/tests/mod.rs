//! Native client RPC behavior, using the SDK's in-process callback override.
use super::*;
use crate::tests::fixture::Fixture;
use futures::FutureExt;
use integrations::strava::events::{StravaEvent, StravaOperation};
use prost::Message;
use sea_orm::{DatabaseBackend, MockDatabase};
use std::sync::{Arc, Mutex};
use temporalio_client::callback_based::{CallbackBasedGrpcService, GrpcRequest, GrpcSuccessResponse};
use temporalio_common::protos::temporal::api::workflowservice::v1::{
    StartWorkflowExecutionRequest, StartWorkflowExecutionResponse,
};
use temporalio_workflow::common::WorkflowDefinition;

#[derive(Default)]
struct MockTemporal {
    requests: Mutex<Vec<GrpcRequest>>,
    failure: Option<(&'static str, tonic::Code)>,
}

impl MockTemporal {
    async fn client(self: &Arc<Self>) -> Client {
        let mock = self.clone();
        let mut options = connection_options(&config()).unwrap();
        options.dns_load_balancing = None;
        options.service_override = Some(CallbackBasedGrpcService {
            callback: Arc::new(move |request| {
                let mock = mock.clone();
                async move { mock.reply(request) }.boxed()
            }),
        });
        connect_with_options(options, &config().namespace).await.unwrap()
    }

    fn reply(&self, request: GrpcRequest) -> Result<GrpcSuccessResponse, tonic::Status> {
        let rpc = request.rpc.clone();
        self.requests.lock().unwrap().push(request);
        if let Some((failed_rpc, code)) = self.failure {
            if rpc == failed_rpc {
                if code == tonic::Code::AlreadyExists {
                    return Err(bike_core::test_support::temporal_client::already_started());
                }
                return Err(tonic::Status::new(code, "fixture Temporal failure"));
            }
        }
        let proto = match rpc.as_str() {
            "GetSystemInfo" => vec![],
            "StartWorkflowExecution" => StartWorkflowExecutionResponse {
                run_id: "fixture-native-run".into(),
                ..Default::default()
            }
            .encode_to_vec(),
            _ => panic!("Unexpected Temporal RPC {rpc}"),
        };
        Ok(GrpcSuccessResponse {
            headers: Default::default(),
            proto,
        })
    }
}

fn config() -> TemporalConfig {
    TemporalConfig {
        address: "http://fixture.invalid:7233".into(),
        namespace: "bike".into(),
        task_queue: "bike-strava".into(),
    }
}

fn row(event: &StravaEvent) -> bike_core::entities::strava_events::Model {
    let mut fixture = Fixture::new(event.operation);
    fixture.event = event.clone();
    fixture.row(integrations::strava::native::ActivityStatus::Pending)
}

#[tokio::test]
async fn each_operation_starts_its_own_workflow_with_correlated_input_and_duplicate_protection() {
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    for (operation, workflow) in [
        (StravaOperation::Create, CreateActivity::run.name()),
        (StravaOperation::Update, UpdateActivity::run.name()),
        (StravaOperation::Upsert, UpsertActivity::run.name()),
        (StravaOperation::Delete, DeleteActivity::run.name()),
        (StravaOperation::Sync, SyncActivities::run.name()),
        (StravaOperation::Backfill, BackfillActivities::run.name()),
        (StravaOperation::Disconnect, DisconnectStrava::run.name()),
    ] {
        let fixture = Fixture::new(operation);
        assert_eq!(
            start_event(&client, &config(), fixture.event.clone())
                .await
                .unwrap()
                .as_deref(),
            Some("fixture-native-run")
        );
        let requests = mock.requests.lock().unwrap();
        let request = StartWorkflowExecutionRequest::decode(requests.last().unwrap().proto.clone()).unwrap();
        assert_eq!(request.workflow_type.unwrap().name, workflow);
        assert!(request.retry_policy.is_none());
        assert_eq!(request.workflow_id, fixture.event.workflow_id());
        assert_eq!(request.task_queue.unwrap().name, config().task_queue);
        assert_eq!(request.namespace, config().namespace);
        assert_eq!(
            request.workflow_id_reuse_policy,
            temporalio_common::protos::temporal::api::enums::v1::WorkflowIdReusePolicy::RejectDuplicate as i32
        );
        let input: StravaEvent = serde_json::from_slice(&request.input.unwrap().payloads[0].data).unwrap();
        assert_eq!(input.strava_event_id, fixture.event.strava_event_id);
        assert_eq!(input.correlation_id, fixture.event.correlation_id);
        let attributes = request.search_attributes.unwrap().indexed_fields;
        assert_eq!(
            attributes.len(),
            if fixture.event.strava_activity_id.is_some() {
                6
            } else {
                5
            }
        );
        assert_eq!(
            serde_json::from_slice::<String>(&attributes["BikeCustomerId"].data).unwrap(),
            fixture.event.bike_customer_id.to_string()
        );
        assert_eq!(
            serde_json::from_slice::<String>(&attributes["CorrelationId"].data).unwrap(),
            fixture.event.correlation_id.to_string()
        );
    }
}

#[tokio::test]
async fn duplicate_start_is_successful_but_other_start_failures_retry() {
    let event = Fixture::new(StravaOperation::Upsert).event;
    for (code, success) in [
        (tonic::Code::AlreadyExists, true),
        (tonic::Code::PermissionDenied, false),
    ] {
        let mock = Arc::new(MockTemporal {
            failure: Some(("StartWorkflowExecution", code)),
            ..Default::default()
        });
        let result = start_event(&mock.client().await, &config(), event.clone()).await;
        assert_eq!(result.is_ok(), success);
    }
}

#[tokio::test]
async fn direct_submission_acknowledges_success_and_duplicates_but_returns_start_failures() {
    for code in [
        None,
        Some(tonic::Code::AlreadyExists),
        Some(tonic::Code::PermissionDenied),
    ] {
        let mock = Arc::new(MockTemporal {
            failure: code.map(|code| ("StartWorkflowExecution", code)),
            ..Default::default()
        });
        let event = Fixture::new(StravaOperation::Upsert).event;
        let row = row(&event);
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![row.clone()]])
            .into_connection();
        let result =
            integrations::strava::events::submission::submit_accepted(&mock.client().await, &config(), &db, row).await;
        let log = format!("{:?}", db.into_transaction_log());
        if code == Some(tonic::Code::PermissionDenied) {
            assert!(result.is_err());
            assert!(!log.contains("UPDATE"));
        } else {
            result.unwrap();
            assert!(log.contains("submitted_at"));
        }
        assert!(!log.contains("SELECT"));
    }
}

#[tokio::test]
async fn malformed_stored_event_is_rejected_without_contacting_temporal() {
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    let mut row = row(&Fixture::new(StravaOperation::Upsert).event);
    row.event = serde_json::json!({"broken":"input"});
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row.clone()]])
        .into_connection();
    assert!(
        integrations::strava::events::submission::submit_accepted(&client, &config(), &db, row)
            .await
            .is_err()
    );
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
    assert!(db.into_transaction_log().is_empty());
}

#[tokio::test]
async fn submission_record_failure_is_returned_after_a_successful_start() {
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([sea_orm::DbErr::Custom("fixture database failed".into())])
        .into_connection();
    let row = row(&Fixture::new(StravaOperation::Upsert).event);
    assert!(
        integrations::strava::events::submission::submit_accepted(&client, &config(), &db, row)
            .await
            .is_err()
    );
    assert_eq!(
        mock.requests.lock().unwrap().last().unwrap().rpc,
        "StartWorkflowExecution"
    );
}
