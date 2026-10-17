use super::*;
use serde::{de::DeserializeOwned, Serialize};

#[async_trait::async_trait]
pub trait ActivityStorage: Send + Sync {
    async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError>;
    async fn write(&self, key: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError>;
    async fn remove(&self, path: &str) -> Result<(), WorkflowError>;
}

impl dyn ActivityStorage {
    pub async fn read_json<T: DeserializeOwned>(&self, object: &ObjectRef) -> Result<T, WorkflowError> {
        serde_json::from_slice(&self.read(object).await?).map_err(|error| WorkflowError::bad_request(error.to_string()))
    }

    pub async fn transform<T: Serialize>(&self, key: &str, value: &T) -> Result<ObjectRef, WorkflowError> {
        let bytes = serde_json::to_vec(value).map_err(|error| WorkflowError::internal(error.to_string()))?;
        self.write(key, &bytes).await
    }
}

#[async_trait::async_trait]
impl ActivityStorage for ObjectStore {
    async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
        ObjectStore::read(self, object).await
    }

    async fn write(&self, key: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
        ObjectStore::write(self, key, bytes).await
    }

    async fn remove(&self, path: &str) -> Result<(), WorkflowError> {
        ObjectStore::remove(self, path).await
    }
}

#[async_trait::async_trait]
pub trait StravaProvider: Send + Sync {
    async fn activity(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError>;
    async fn streams(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError>;
    async fn page(&self, input: &PageInput) -> Result<Vec<StravaActivitySummary>, WorkflowError>;
    async fn revoke(&self, event: &StravaEvent) -> Result<(), WorkflowError>;
}

pub(super) struct ProviderClient {
    pub db: DatabaseConnection,
    pub store: ObjectStore,
}

impl ProviderClient {
    async fn access(&self, event: &StravaEvent) -> Result<(StravaApiClient, String), WorkflowError> {
        let connection = strava_connections::Model::for_event(&self.db, event).await?;
        let client = StravaApiClient::new(&self.db, Config::get())?;
        let connection = super::super::ensure_fresh_access_token(&self.db, &client, connection).await?;
        Ok((client, super::super::credentials::open(&connection.access_token)?))
    }

    fn archive(&self, event: &StravaEvent, resource: &str) -> RequestArchive {
        RequestArchive {
            store: self.store.clone(),
            key: object_key(event, resource),
        }
    }

    async fn response(&self, archive: &RequestArchive) -> Result<ObjectRef, WorkflowError> {
        self.store
            .response(&archive.key)
            .await?
            .map(|response| response.body)
            .ok_or_else(|| WorkflowError::internal("Strava response was not retained"))
    }
}

#[async_trait::async_trait]
impl StravaProvider for ProviderClient {
    async fn activity(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
        let (client, token) = self.access(event).await?;
        let archive = self.archive(event, "activity");
        archive
            .clone()
            .scope(client.get_activity(&token, event.activity_id()?))
            .await?;
        self.response(&archive).await
    }

    async fn streams(&self, event: &StravaEvent) -> Result<ObjectRef, WorkflowError> {
        let (client, token) = self.access(event).await?;
        let archive = self.archive(event, "streams");
        archive
            .clone()
            .scope(client.get_activity_streams(&token, event.activity_id()?))
            .await?;
        self.response(&archive).await
    }

    async fn page(&self, input: &PageInput) -> Result<Vec<StravaActivitySummary>, WorkflowError> {
        let (client, token) = self.access(&input.collection.event).await?;
        client
            .list_activity_page(&token, Some(input.collection.window.after), input.page)
            .await?
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| WorkflowError::bad_request(error.to_string()))
    }

    async fn revoke(&self, event: &StravaEvent) -> Result<(), WorkflowError> {
        let (client, token) = self.access(event).await?;
        self.archive(event, "deauthorize")
            .scope(client.deauthorize(&token))
            .await
    }
}

pub(super) fn object_key(event: &StravaEvent, resource: &str) -> String {
    format!("strava/events/{}/{resource}", event.strava_event_id)
}
