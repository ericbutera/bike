//! Native client RPC behavior, using the SDK's in-process callback override.
use super::*;
use crate::tests::fixture::Fixture;
use futures::FutureExt;
use prost::Message;
use sea_orm::{DatabaseBackend, MockDatabase};
use std::sync::{Arc, Mutex};
use temporalio_client::callback_based::{CallbackBasedGrpcService, GrpcRequest, GrpcSuccessResponse};
use temporalio_common::protos::temporal::api::workflowservice::v1::{
    CreateScheduleRequest, StartWorkflowExecutionRequest, StartWorkflowExecutionResponse,
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
                return Err(tonic::Status::new(code, "fixture Temporal failure"));
            }
        }
        let proto = match rpc.as_str() {
            "GetSystemInfo" | "CreateSchedule" => vec![],
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
    fixture.row(bike_core::strava::native::ActivityStatus::Pending)
}

#[test]
fn configuration_uses_defaults_or_explicit_values_without_mutating_process_environment() {
    let defaults = TemporalConfig::from_lookup(|_| None);
    assert_eq!(defaults.address, "http://temporal:7233");
    assert_eq!(defaults.namespace, "bike");
    assert_eq!(defaults.task_queue, "bike-strava");
    let custom = TemporalConfig::from_lookup(|name| Some(name.to_lowercase()));
    assert_eq!(custom.address, "temporal_address");
    assert_eq!(custom.namespace, "temporal_namespace");
    assert_eq!(custom.task_queue, "temporal_task_queue");
    let options = connection_options(&config()).unwrap();
    assert_eq!(options.identity, "bike-strava-temporal");
    assert_eq!(options.connect_timeout, Some(Duration::from_secs(10)));
}

#[tokio::test]
async fn invalid_address_is_rejected_before_connection() {
    let mut config = config();
    config.address = "not a URL".into();
    assert!(connect(&config).await.is_err());
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
async fn schedule_uses_native_scan_every_six_hours_and_skips_overlap() {
    let mock = Arc::new(MockTemporal::default());
    ensure_schedule(&mock.client().await, &config()).await.unwrap();
    let requests = mock.requests.lock().unwrap();
    let request = CreateScheduleRequest::decode(requests.last().unwrap().proto.clone()).unwrap();
    assert_eq!(request.schedule_id, "bike-strava-sync");
    let schedule = request.schedule.unwrap();
    assert_eq!(schedule.spec.unwrap().interval[0].interval.unwrap().seconds, 6 * 3600);
    assert_eq!(
        schedule.policies.unwrap().overlap_policy,
        temporalio_common::protos::temporal::api::enums::v1::ScheduleOverlapPolicy::Skip as i32
    );
    let action = schedule.action.unwrap().action.unwrap();
    let temporalio_common::protos::temporal::api::schedule::v1::schedule_action::Action::StartWorkflow(action) = action;
    assert_eq!(action.workflow_type.unwrap().name, ScheduledSync::run.name());
    assert_eq!(action.task_queue.unwrap().name, config().task_queue);
}

#[tokio::test]
async fn existing_schedule_is_kept_and_other_creation_errors_are_reported() {
    for (code, success) in [
        (tonic::Code::AlreadyExists, true),
        (tonic::Code::PermissionDenied, false),
    ] {
        let mock = Arc::new(MockTemporal {
            failure: Some(("CreateSchedule", code)),
            ..Default::default()
        });
        assert_eq!(ensure_schedule(&mock.client().await, &config()).await.is_ok(), success);
    }
}

#[tokio::test]
async fn handoff_marks_success_and_duplicate_submitted_and_keeps_failures_durable() {
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
            .append_query_results([vec![row.clone()], vec![row]])
            .into_connection();
        dispatch_pending(&mock.client().await, &config(), &db).await.unwrap();
        let log = format!("{:?}", db.into_transaction_log());
        if code == Some(tonic::Code::PermissionDenied) {
            assert!(log.contains("handoff_error"));
            assert!(log.contains("next_attempt_at"));
        } else {
            assert!(log.contains("submitted_at"));
        }
    }
}

#[tokio::test]
async fn malformed_stored_event_retries_without_contacting_temporal() {
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    let mut row = row(&Fixture::new(StravaOperation::Upsert).event);
    row.event = serde_json::json!({"broken":"input"});
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![row.clone()]])
        .into_connection();
    dispatch_event(&client, &config(), &db, row).await.unwrap();
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
    assert!(format!("{:?}", db.into_transaction_log()).contains("handoff_error"));
}

#[tokio::test]
async fn database_handoff_failure_is_reported_for_safe_worker_restart() {
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_errors([sea_orm::DbErr::Custom("fixture database failed".into())])
        .into_connection();
    assert!(dispatch_pending(&client, &config(), &db).await.is_err());
}

#[tokio::test]
async fn native_worker_registers_without_contacting_a_real_service() {
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    let runtime = Runtime::from_current_tokio(Default::default()).unwrap();
    let db = MockDatabase::new(DatabaseBackend::Postgres).into_connection();
    let worker = worker(&runtime, client, &config(), db).unwrap();
    worker.shutdown_handle()();
    assert_eq!(mock.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn dispatcher_samples_backlog_hands_off_an_event_and_reports_database_loss() {
    use sea_orm::Value;
    use std::collections::BTreeMap;
    let mock = Arc::new(MockTemporal::default());
    let client = mock.client().await;
    let row = row(&Fixture::new(StravaOperation::Upsert).event);
    let counts: BTreeMap<String, Value> = [
        ("pending".into(), 1_i64.into()),
        ("oldest_accepted_at".into(), Some(row.accepted_at).into()),
    ]
    .into();
    let db = MockDatabase::new(DatabaseBackend::Postgres)
        .append_query_results([vec![counts]])
        .append_query_results([vec![row.clone()], vec![row]])
        .append_query_errors([sea_orm::DbErr::Custom("fixture database lost".into())])
        .into_connection();
    let error = dispatch_events(client, config(), db).await.unwrap_err();
    assert!(error.to_string().contains("fixture database lost"));
    assert_eq!(
        mock.requests.lock().unwrap().last().unwrap().rpc,
        "StartWorkflowExecution"
    );
    let metrics = prometheus::gather();
    let pending = metrics
        .iter()
        .find(|family| family.name() == "bike_rust_strava_handoff_pending_events")
        .unwrap();
    assert_eq!(pending.get_metric()[0].get_gauge().value(), 1.0);
}
