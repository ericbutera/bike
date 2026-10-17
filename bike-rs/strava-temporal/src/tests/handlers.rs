use super::{fixture::Fixture, logging::Logs};
use crate::{activities::StravaActivities, workflows::*};
use bike_core::{
    strava::{
        events::{StravaEvent, StravaOperation},
        native::*,
        objects::{checksum, ObjectRef},
    },
    workflow_error::WorkflowError,
};
use sea_orm::{DatabaseBackend, MockDatabase};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use temporalio_sdk::{
    activities::ActivityError,
    testing::{ActivityEnvironment, ActivityEnvironmentError, TestActivityInfoOptions},
};
use temporalio_workflow::common::WorkflowDefinition;

#[derive(Default)]
struct Storage {
    objects: Mutex<HashMap<String, Vec<u8>>>,
    traces: Mutex<Vec<bike_core::observability::TraceContextCarrier>>,
    removed: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ActivityStorage for Storage {
    async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
        if let Some(carrier) = bike_core::observability::inject_current_trace_context() {
            self.traces.lock().unwrap().push(carrier);
        }
        self.objects
            .lock()
            .unwrap()
            .get(&object.path)
            .cloned()
            .ok_or_else(|| WorkflowError::internal("Fixture source missing"))
    }
    async fn write(&self, key: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
        let path = format!("{key}/{}.json", checksum(bytes));
        self.objects.lock().unwrap().insert(path.clone(), bytes.to_vec());
        Ok(ObjectRef {
            path,
            sha256: checksum(bytes),
            bytes: bytes.len(),
        })
    }
    async fn remove(&self, path: &str) -> Result<(), WorkflowError> {
        self.removed.lock().unwrap().push(path.into());
        self.objects.lock().unwrap().remove(path);
        Ok(())
    }
}

struct Provider {
    storage: Arc<Storage>,
    calls: Mutex<Vec<&'static str>>,
}

impl Provider {
    async fn response(
        &self,
        event: &StravaEvent,
        resource: &'static str,
        bytes: &[u8],
    ) -> Result<ObjectRef, WorkflowError> {
        self.calls.lock().unwrap().push(resource);
        self.storage
            .write(&format!("fixture/{}/{resource}", event.strava_event_id), bytes)
            .await
    }
}
#[async_trait::async_trait]
impl StravaProvider for Provider {
    async fn activity(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
        self.response(event, "activity", include_bytes!("fixtures/activity.json"))
            .await
    }
    async fn streams(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
        self.response(event, "streams", include_bytes!("fixtures/streams.json"))
            .await
    }
    async fn page(
        &self,
        _input: &PageInput,
    ) -> Result<Vec<bike_core::strava::provider_payload::StravaActivitySummary>, WorkflowError> {
        self.calls.lock().unwrap().push("page");
        Ok(vec![
            serde_json::from_slice(include_bytes!("fixtures/activity.json")).unwrap()
        ])
    }
    async fn revoke(&self, _event: &StravaEvent) -> Result<(), WorkflowError> {
        self.calls.lock().unwrap().push("revoke");
        Ok(())
    }
}

fn environment(event: &StravaEvent, storage: Arc<Storage>) -> ActivityEnvironment {
    environment_with_database(event, storage, MockDatabase::new(DatabaseBackend::Postgres)).0
}

fn environment_with_database(
    event: &StravaEvent,
    storage: Arc<Storage>,
    db: MockDatabase,
) -> (ActivityEnvironment, Arc<Provider>) {
    let provider = Arc::new(Provider {
        storage: storage.clone(),
        calls: Mutex::default(),
    });
    let operations = StravaOperations::with_dependencies(db.into_connection(), provider.clone(), storage);
    let workflow = match event.operation {
        StravaOperation::Create | StravaOperation::Update | StravaOperation::Upsert => UpsertActivity::run.name(),
        StravaOperation::Delete => DeleteActivity::run.name(),
        StravaOperation::Sync => SyncActivities::run.name(),
        StravaOperation::Backfill => BackfillActivities::run.name(),
        StravaOperation::Disconnect => DisconnectStrava::run.name(),
    };
    let info = TestActivityInfoOptions::builder()
        .namespace("bike")
        .workflow_run_id(Some("child-native-run".into()))
        .workflow_type(Some(workflow.into()))
        .workflow_id(Some(event.workflow_id()))
        .deadline(None)
        .build();
    let environment = ActivityEnvironment::builder_with_default()
        .info(info)
        .register_activities(StravaActivities::with_operations(operations))
        .build();
    (environment, provider)
}

fn storage(fixture: &Fixture) -> Arc<Storage> {
    Arc::new(Storage {
        traces: Mutex::default(),
        removed: Mutex::default(),
        objects: Mutex::new(
            [
                (
                    fixture.activity().activity.path,
                    include_bytes!("fixtures/activity.json").to_vec(),
                ),
                (
                    fixture.sources().streams.path,
                    include_bytes!("fixtures/streams.json").to_vec(),
                ),
            ]
            .into(),
        ),
    })
}

mod operations;

#[tokio::test]
async fn child_handler_restores_remote_trace_and_logs_its_native_execution() {
    use opentelemetry::trace::TracerProvider as _;
    use tracing::instrument::WithSubscriber;
    use tracing_subscriber::layer::SubscriberExt;

    let parent = Fixture::new(StravaOperation::Sync);
    let mut fixture = Fixture::new(StravaOperation::Upsert);
    fixture.event = parent.event.for_activity(fixture.activity_id, StravaOperation::Upsert);
    let storage = storage(&fixture);
    let logs = Logs::default();
    opentelemetry::global::set_text_map_propagator(opentelemetry_sdk::propagation::TraceContextPropagator::new());
    let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder().build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("strava-unit")))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(logs.clone())
                .with_ansi(false),
        )
        .with(tracing_subscriber::filter::dynamic_filter_fn(|metadata, _| {
            *metadata.level() <= tracing::Level::INFO
        }));
    environment(&fixture.event, storage.clone())
        .run(StravaActivities::normalize_activity, fixture.sources())
        .with_subscriber(subscriber)
        .await
        .unwrap();

    let rows = logs.rows();
    assert_eq!(rows.len(), 2);
    for row in rows {
        let fields = &row["fields"];
        assert_eq!(fields["correlation_id"], parent.event.correlation_id.to_string());
        assert_eq!(fields["parent_event_id"], parent.event.strava_event_id.to_string());
        assert_eq!(fields["strava_activity_id"], fixture.activity_id);
        assert_eq!(fields["bike_customer_id"], parent.event.bike_customer_id.to_string());
        assert_eq!(fields["request_id"], "fixture-request");
        assert_eq!(fields["trace_id"], "12345678901234567890123456789012");
        assert_eq!(fields["temporal_workflow_id"], fixture.event.workflow_id());
        assert_eq!(fields["temporal_run_id"], "child-native-run");
    }
    let traces = storage.traces.lock().unwrap();
    assert_eq!(traces.len(), 2);
    for carrier in traces.iter() {
        let parts: Vec<_> = carrier["traceparent"].split('-').collect();
        assert_eq!(parts[1], "12345678901234567890123456789012");
        assert_ne!(parts[2], "1234567890123456");
        assert_eq!(carrier["tracestate"], "bike=fixture");
    }
}

#[tokio::test]
async fn registered_normalization_handler_returns_real_fixture_transforms() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let storage = storage(&fixture);
    let result = environment(&fixture.event, storage.clone())
        .run(StravaActivities::normalize_activity, fixture.sources())
        .await
        .unwrap();
    assert_eq!(result.event.strava_activity_id, Some(fixture.activity_id));
    let objects = storage.objects.lock().unwrap();
    let parsed: bike_core::activity_parser::ParsedActivityData =
        serde_json::from_slice(objects.get(&result.parsed.unwrap().path).unwrap()).unwrap();
    assert_eq!(parsed.draft.title, "Fixture ride");
    assert!(objects.contains_key(&result.source.path));
}

#[tokio::test]
async fn large_provider_streams_are_normalized_in_storage_and_return_only_references() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let storage = storage(&fixture);
    let mut input = fixture.sources();
    let points = 150_000;
    let streams = serde_json::json!({
        "time": {"data": (0..points).collect::<Vec<_>>()},
        "distance": {"data": (0..points).map(|i| i * 3).collect::<Vec<_>>()},
        "latlng": {"data": (0..points).map(|i| [45.0, -120.0 + f64::from(i) * 0.00001]).collect::<Vec<_>>()}
    });
    let bytes = serde_json::to_vec(&streams).unwrap();
    input.streams = storage.write("large-streams", &bytes).await.unwrap();
    assert!(input.streams.bytes > 2 * 1024 * 1024);
    let result = environment(&fixture.event, storage.clone())
        .run(StravaActivities::normalize_activity, input.clone())
        .await
        .unwrap();
    let objects = storage.objects.lock().unwrap();
    assert_eq!(objects[&input.streams.path], bytes);
    assert!(result.source.bytes > 2 * 1024 * 1024);
    assert!(result.parsed.as_ref().unwrap().bytes > 2 * 1024 * 1024);
    assert!(serde_json::to_vec(&result).unwrap().len() < 4096);
    bike_core::strava::payload::validate_temporal(&result).unwrap();
}

#[tokio::test]
async fn registered_handler_rejects_wrong_workflow_identity_before_external_work() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let environment = environment(&fixture.event, Arc::new(Storage::default()));
    let mut input = fixture.sources();
    input.event.strava_event_id = uuid::Uuid::from_u128(99);
    let result = environment.run(StravaActivities::normalize_activity, input).await;
    assert!(matches!(
        result,
        Err(ActivityEnvironmentError::Activity(ActivityError::Application(_)))
    ));
}

#[tokio::test]
async fn registered_handler_rejects_wrong_operation_before_external_work() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let environment = environment(&fixture.event, Arc::new(Storage::default()));
    let mut input = fixture.sources();
    input.event.operation = StravaOperation::Delete;
    assert!(environment
        .run(StravaActivities::normalize_activity, input)
        .await
        .is_err());
}

#[tokio::test]
async fn registered_handler_reports_storage_failure_instead_of_completing() {
    let fixture = Fixture::new(StravaOperation::Upsert);
    let environment = environment(&fixture.event, Arc::new(Storage::default()));
    assert!(environment
        .run(StravaActivities::normalize_activity, fixture.sources())
        .await
        .is_err());
}
