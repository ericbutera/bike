use super::fixture::Fixture;
use crate::{activities::StravaActivities, workflows::*};
use bike_core::workflow_error::WorkflowError;
use integrations::strava::{
    events::{StravaEvent, StravaOperation},
    native::*,
    objects::{checksum, ObjectRef},
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
    removed: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl ActivityStorage for Storage {
    async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
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
    ) -> Result<Vec<integrations::strava::provider_payload::StravaActivitySummary>, WorkflowError> {
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
    let operations = StravaOperations::with_dependencies(
        db.into_connection(),
        provider.clone(),
        storage,
        bike_core::test_support::object_storage(bike_core::object_store::ObjectOwner::Strava).0,
    );
    let workflow = if event.source_version.is_some() {
        ReconcileActivity::run.name()
    } else {
        match event.operation {
            StravaOperation::Create | StravaOperation::Update | StravaOperation::Upsert => UpsertActivity::run.name(),
            StravaOperation::Delete => DeleteActivity::run.name(),
            StravaOperation::Sync => SyncActivities::run.name(),
            StravaOperation::Backfill => BackfillActivities::run.name(),
            StravaOperation::Disconnect => DisconnectStrava::run.name(),
        }
    };
    let workflow_id = if event.source_version.is_some() {
        event.source_workflow_id().unwrap()
    } else {
        event.workflow_id()
    };
    let info = TestActivityInfoOptions::builder()
        .namespace("bike")
        .workflow_run_id(Some("child-native-run".into()))
        .workflow_type(Some(workflow.into()))
        .workflow_id(Some(workflow_id))
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
async fn large_provider_streams_are_retained_in_storage_and_return_only_references() {
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
        .run(StravaActivities::retain_activity, input.clone())
        .await
        .unwrap();
    let objects = storage.objects.lock().unwrap();
    assert_eq!(objects[&input.streams.path], bytes);
    assert!(result.source.bytes > 2 * 1024 * 1024);
    assert!(serde_json::to_vec(&result).unwrap().len() < 4096);
    integrations::strava::payload::validate_temporal(&result).unwrap();
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
