use super::*;
use crate::strava::{
    objects::{checksum, ObjectRef},
    provider_payload::StoredStravaProviderPayload,
};
use axum::http::StatusCode;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(super) struct Storage {
    pub objects: Mutex<HashMap<String, Vec<u8>>>,
    pub removed: Mutex<Vec<String>>,
    pub failure: Mutex<bool>,
}

#[async_trait::async_trait]
impl ActivityStorage for Storage {
    async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
        self.check()?;
        self.objects
            .lock()
            .unwrap()
            .get(&object.path)
            .cloned()
            .ok_or_else(|| WorkflowError::internal("Missing fixture object"))
    }

    async fn write(&self, key: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
        self.check()?;
        let sha256 = checksum(bytes);
        let path = format!("{key}/{sha256}.json");
        self.objects.lock().unwrap().insert(path.clone(), bytes.to_vec());
        Ok(ObjectRef {
            path,
            sha256,
            bytes: bytes.len(),
        })
    }

    async fn remove(&self, path: &str) -> Result<(), WorkflowError> {
        self.check()?;
        self.removed.lock().unwrap().push(path.into());
        self.objects.lock().unwrap().remove(path);
        Ok(())
    }
}

impl Storage {
    async fn write_json<T: serde::Serialize>(&self, key: &str, value: &T) -> Result<ObjectRef, WorkflowError> {
        let storage: &dyn ActivityStorage = self;
        storage.transform(key, value).await
    }

    fn check(&self) -> Result<(), WorkflowError> {
        if *self.failure.lock().unwrap() {
            return Err(WorkflowError::internal("Fixture storage failed"));
        }
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ProviderCall {
    Activity,
    Streams,
    Page { after: i64, page: i32 },
    Revoke,
}

impl ProviderCall {
    fn resource(&self) -> &'static str {
        match self {
            Self::Activity => "activity",
            Self::Streams => "streams",
            Self::Page { .. } => "page",
            Self::Revoke => "deauthorize",
        }
    }
}

pub(super) struct Provider {
    pub payload: StoredStravaProviderPayload,
    pub storage: Arc<Storage>,
    pub calls: Mutex<Vec<ProviderCall>>,
    pub failure: Mutex<Option<StatusCode>>,
    pub empty_page: bool,
}

impl Provider {
    async fn response<T: serde::Serialize>(
        &self,
        event: &StravaEvent,
        call: ProviderCall,
        payload: &T,
    ) -> Result<ObjectRef, WorkflowError> {
        let key = object_key(event, call.resource());
        self.record(call)?;
        self.storage.write_json(&key, payload).await
    }

    fn record(&self, call: ProviderCall) -> Result<(), WorkflowError> {
        self.calls.lock().unwrap().push(call);
        if let Some(status) = *self.failure.lock().unwrap() {
            return Err(WorkflowError {
                status,
                message: "Fixture provider failed".into(),
                errors: None,
                retry_at: (status == StatusCode::TOO_MANY_REQUESTS)
                    .then_some(chrono::DateTime::from_timestamp(1790856300, 0).unwrap()),
            });
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl StravaProvider for Provider {
    async fn activity(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
        self.response(event, ProviderCall::Activity, &self.payload.activity)
            .await
    }

    async fn streams(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
        self.response(event, ProviderCall::Streams, &self.payload.streams).await
    }

    async fn page(&self, input: &PageInput) -> Result<Vec<StravaActivitySummary>, WorkflowError> {
        self.record(ProviderCall::Page {
            after: input.collection.window.after,
            page: input.page,
        })?;
        Ok(if self.empty_page {
            vec![]
        } else {
            vec![self.payload.activity.clone()]
        })
    }

    async fn revoke(&self, _event: &StravaEvent) -> Result<(), WorkflowError> {
        self.record(ProviderCall::Revoke)
    }
}

pub(super) struct HandlerFixture {
    pub domain: Fixture,
    pub provider: Arc<Provider>,
    pub storage: Arc<Storage>,
}

impl HandlerFixture {
    pub fn new() -> Self {
        let payload: StoredStravaProviderPayload = serde_json::from_slice(crate::strava::fixtures::REAL_RIDE).unwrap();
        let mut domain = Fixture::new();
        domain.event.strava_activity_id = Some(payload.activity.id);
        let storage = Arc::new(Storage::default());
        let provider = Arc::new(Provider {
            payload,
            storage: storage.clone(),
            calls: Mutex::new(vec![]),
            failure: Mutex::new(None),
            empty_page: false,
        });
        Self {
            domain,
            provider,
            storage,
        }
    }

    pub fn connection_mock(&self) -> MockDatabase {
        MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![self.domain.connection.clone()]])
            .append_query_results([vec![self.domain.customer.clone()]])
    }

    pub fn operations(&self, db: MockDatabase) -> StravaOperations {
        StravaOperations::with_dependencies(db.into_connection(), self.provider.clone(), self.storage.clone())
    }

    pub fn activity_rebuild_mock(&self, activity: &activities::Model) -> MockDatabase {
        use bike_core::{
            entities::{activity_award_outputs, activity_earned_awards, segment_efforts},
            test_support as fixture,
        };
        fixture::postgres_mock()
            .append_query_results([vec![self.domain.connection.clone()]])
            .append_query_results([vec![self.domain.activity_state()]])
            .append_query_results([vec![activity.clone()]])
            .append_query_results([fixture::segment_id_rows(&[4])])
            .append_query_results([vec![segment_efforts::Model {
                user_id: activity.user_id,
                activity_id: activity.id,
                segment_id: 4,
                ..fixture::segment_effort()
            }]])
            .append_query_results([fixture::segment_id_rows(&[])])
            .append_query_results([vec![fixture::activity_id_row()]])
            .append_query_results([vec![fixture::activity_analytics()]])
            .append_query_results([Vec::<segment_efforts::Model>::new()])
            .append_query_results([Vec::<activity_award_outputs::Model>::new()])
            .append_query_results([Vec::<activity_earned_awards::Model>::new()])
    }

    pub async fn sources(&self) -> ActivitySources {
        let activity = self
            .storage
            .write_json("fixture/activity", &self.provider.payload.activity)
            .await
            .unwrap();
        let streams = self
            .storage
            .write_json("fixture/streams", &self.provider.payload.streams)
            .await
            .unwrap();
        ActivitySources {
            event: self.domain.event.clone(),
            activity,
            streams,
        }
    }
}
