//! Backend-independent objects, immutable references, and retention policy.
mod backend;
pub mod retention;

use crate::workflow_error::WorkflowError;
use ::object_store::{path::Path as ObjectPath, ObjectStore as Storage, ObjectStoreExt, PutMode};
use backend::Backend;
use chrono::{DateTime, Utc};
use retention::{ReferenceInventory, RetentionDecision, RetentionPolicy, RetentionRecord};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRef {
    /// Logical storage key, retained under its existing serialized field name.
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectOwner {
    Strava,
    BikeTasks,
    ActivityImports,
}

impl ObjectOwner {
    pub fn prefix(self) -> &'static str {
        match self {
            Self::Strava => "strava",
            Self::BikeTasks => "bike/tasks",
            Self::ActivityImports => "activity-imports",
        }
    }

    pub fn owns(self, key: &str) -> bool {
        if key.starts_with('/') {
            return false;
        }
        ObjectPath::parse(key).is_ok_and(|key| key.as_ref().starts_with(&format!("{}/", self.prefix())))
    }
}

#[derive(Clone)]
pub struct ObjectStore {
    backend: Arc<Backend>,
    owner: ObjectOwner,
    retention: RetentionPolicy,
}

impl ObjectStore {
    /// Composition roots inject a backend. Business code sees logical keys only.
    pub fn new(backend: Arc<dyn Storage>, owner: ObjectOwner) -> Self {
        Self::with_backend(Backend::Ready(backend), owner)
    }

    pub fn filesystem(root: impl AsRef<std::path::Path>, owner: ObjectOwner) -> Self {
        Self::with_backend(Backend::filesystem(root), owner)
    }

    fn with_backend(backend: Backend, owner: ObjectOwner) -> Self {
        Self {
            backend: Arc::new(backend),
            owner,
            retention: RetentionPolicy::default(),
        }
    }

    pub fn with_retention_policy(mut self, policy: RetentionPolicy) -> Self {
        self.retention = policy;
        self
    }

    pub fn retention_decision(
        &self,
        record: &RetentionRecord,
        now: DateTime<Utc>,
        inventory: ReferenceInventory,
    ) -> RetentionDecision {
        record.decision(self.owner, now, &self.retention, inventory)
    }

    pub async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
        let bytes = self
            .read_bytes(&object.path)
            .await?
            .ok_or_else(|| WorkflowError::not_found(format!("Saved object {} is unavailable", object.path)))?;
        if checksum(&bytes) != object.sha256 || bytes.len() != object.bytes {
            return Err(WorkflowError::invalid_data(format!(
                "Saved object {} failed checksum verification",
                object.path
            )));
        }
        Ok(bytes)
    }

    pub async fn read_json<T: DeserializeOwned>(&self, object: &ObjectRef) -> Result<T, WorkflowError> {
        serde_json::from_slice(&self.read(object).await?).map_err(invalid_json)
    }

    pub async fn write(&self, prefix: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
        self.key(prefix)?;
        let hash = checksum(bytes);
        let object = ObjectRef {
            path: format!("{prefix}/{hash}.json"),
            sha256: hash,
            bytes: bytes.len(),
        };
        self.create(&object.path, bytes).await?;
        self.read(&object).await?;
        Ok(object)
    }

    pub async fn transform<T: Serialize>(&self, prefix: &str, value: &T) -> Result<ObjectRef, WorkflowError> {
        self.write(prefix, &serde_json::to_vec(value).map_err(json_error)?)
            .await
    }

    /// Named descriptors use the same backend as content-addressed bytes.
    pub async fn load_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, WorkflowError> {
        self.read_bytes(key)
            .await?
            .map(|bytes| serde_json::from_slice(&bytes).map_err(invalid_json))
            .transpose()
    }

    /// Return the winning value if concurrent creators race; never overwrite it.
    pub async fn create_json<T: Serialize + DeserializeOwned>(&self, key: &str, value: &T) -> Result<T, WorkflowError> {
        self.create(key, &serde_json::to_vec(value).map_err(json_error)?)
            .await?;
        self.load_json(key)
            .await?
            .ok_or_else(|| WorkflowError::internal("Object descriptor was not retained"))
    }

    pub async fn reference(&self, key: &str) -> Result<Option<ObjectRef>, WorkflowError> {
        let reference = self.load_json::<ObjectRef>(&format!("{key}/reference.json")).await?;
        if let Some(reference) = &reference {
            self.read(reference).await?;
        }
        Ok(reference)
    }

    pub async fn save_reference(&self, key: &str, reference: &ObjectRef) -> Result<(), WorkflowError> {
        self.read(reference).await?;
        let saved = self.create_json(&format!("{key}/reference.json"), reference).await?;
        if saved != *reference {
            return Err(WorkflowError::conflict("Saved source reference is immutable"));
        }
        Ok(())
    }

    /// Explicit legacy domain deletion. TTL cleanup must use the coordinated
    /// reference-aware sweeper; eligibility alone cannot call this operation.
    pub(crate) async fn remove(&self, key: &str) -> Result<(), WorkflowError> {
        let key = self.key(key)?;
        match self.backend.storage().await?.delete(&key).await {
            Ok(()) | Err(::object_store::Error::NotFound { .. }) => Ok(()),
            Err(error) => Err(storage_error(error)),
        }
    }

    async fn read_bytes(&self, key: &str) -> Result<Option<Vec<u8>>, WorkflowError> {
        let key = self.key(key)?;
        let result = match self.backend.storage().await?.get(&key).await {
            Ok(result) => result,
            Err(::object_store::Error::NotFound { .. }) => return Ok(None),
            Err(error) => return Err(storage_error(error)),
        };
        Ok(Some(result.bytes().await.map_err(storage_error)?.to_vec()))
    }

    async fn create(&self, key: &str, bytes: &[u8]) -> Result<(), WorkflowError> {
        let key = self.key(key)?;
        let result = self
            .backend
            .storage()
            .await?
            .put_opts(&key, bytes.to_vec().into(), PutMode::Create.into())
            .await;
        match result {
            Ok(_) | Err(::object_store::Error::AlreadyExists { .. }) => Ok(()),
            Err(error) => Err(storage_error(error)),
        }
    }

    fn key(&self, key: &str) -> Result<ObjectPath, WorkflowError> {
        if !self.owner.owns(key) {
            return Err(WorkflowError::bad_request("Invalid or foreign object key"));
        }
        ObjectPath::parse(key).map_err(|error| WorkflowError::bad_request(error.to_string()))
    }
}

pub fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn storage_error(error: ::object_store::Error) -> WorkflowError {
    WorkflowError::internal(error.to_string())
}

fn json_error(error: serde_json::Error) -> WorkflowError {
    WorkflowError::internal(error.to_string())
}

fn invalid_json(error: serde_json::Error) -> WorkflowError {
    WorkflowError::invalid_data(error.to_string())
}

#[cfg(test)]
mod tests;
