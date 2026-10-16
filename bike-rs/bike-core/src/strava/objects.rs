//! Immutable provider responses and transforms on Bike's existing upload volume.
pub use crate::object_store::{checksum, ObjectRef};
use crate::object_store::{ObjectOwner, ObjectStore as ByteStore};
use crate::workflow_error::WorkflowError;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResponseObject {
    pub body: ObjectRef,
    pub status: u16,
    pub headers: std::collections::BTreeMap<String, String>,
    pub observed_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub encrypted: bool,
}

#[derive(Serialize, Deserialize)]
struct Capture {
    status: u16,
    headers: std::collections::BTreeMap<String, String>,
    observed_at: chrono::DateTime<chrono::Utc>,
    body_base64: String,
    #[serde(default)]
    encrypted: bool,
}

#[derive(Clone)]
pub struct ObjectStore {
    bytes: ByteStore,
}

impl ObjectStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            bytes: ByteStore::filesystem(root, ObjectOwner::Strava),
        }
    }

    pub async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
        self.bytes.read(object).await
    }

    pub async fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        object: &ObjectRef,
    ) -> Result<T, WorkflowError> {
        self.bytes.read_json(object).await
    }

    pub async fn write(&self, prefix: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
        self.bytes.write(prefix, bytes).await
    }

    pub async fn reference(&self, key: &str) -> Result<Option<ObjectRef>, WorkflowError> {
        self.bytes.reference(key).await
    }

    pub async fn save_reference(
        &self,
        key: &str,
        reference: &ObjectRef,
    ) -> Result<(), WorkflowError> {
        self.bytes.save_reference(key, reference).await
    }

    pub async fn response(&self, key: &str) -> Result<Option<ResponseObject>, WorkflowError> {
        if let Some(response) = self
            .bytes
            .load_json::<ResponseObject>(&format!("{key}/manifest.json"))
            .await?
        {
            self.read(&response.body).await?;
            return Ok(Some(response));
        }
        let Some(capture) = self.bytes.load_json(&format!("{key}/capture.json")).await? else {
            return Ok(None);
        };
        self.materialize(key, capture).await.map(Some)
    }

    pub async fn save_response(
        &self,
        key: &str,
        status: u16,
        headers: std::collections::BTreeMap<String, String>,
        bytes: &[u8],
    ) -> Result<ResponseObject, WorkflowError> {
        self.save_capture(key, status, headers, bytes, false).await
    }

    pub async fn save_encrypted_response(
        &self,
        key: &str,
        status: u16,
        headers: std::collections::BTreeMap<String, String>,
        bytes: &[u8],
    ) -> Result<ResponseObject, WorkflowError> {
        let encrypted = crate::strava::credentials::seal(bytes)?;
        self.save_capture(key, status, headers, encrypted.as_bytes(), true)
            .await
    }

    async fn save_capture(
        &self,
        key: &str,
        status: u16,
        headers: std::collections::BTreeMap<String, String>,
        bytes: &[u8],
        encrypted: bool,
    ) -> Result<ResponseObject, WorkflowError> {
        if let Some(saved) = self.response(key).await? {
            return Ok(saved);
        }
        use base64::Engine;
        let capture = Capture {
            status,
            headers,
            observed_at: chrono::Utc::now(),
            body_base64: base64::engine::general_purpose::STANDARD.encode(bytes),
            encrypted,
        };
        let capture = self
            .bytes
            .create_json(&format!("{key}/capture.json"), &capture)
            .await?;
        self.materialize(key, capture).await
    }

    pub async fn response_bytes(
        &self,
        response: &ResponseObject,
    ) -> Result<Vec<u8>, WorkflowError> {
        let bytes = self.read(&response.body).await?;
        if response.encrypted {
            let ciphertext = String::from_utf8(bytes)
                .map_err(|_| WorkflowError::internal("Invalid encrypted Strava response"))?;
            crate::strava::credentials::open(&ciphertext).map(String::into_bytes)
        } else {
            Ok(bytes)
        }
    }

    async fn materialize(
        &self,
        key: &str,
        capture: Capture,
    ) -> Result<ResponseObject, WorkflowError> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(capture.body_base64)
            .map_err(|error| WorkflowError::internal(error.to_string()))?;
        let response = ResponseObject {
            body: self.write(key, &bytes).await?,
            status: capture.status,
            headers: capture.headers,
            observed_at: capture.observed_at,
            encrypted: capture.encrypted,
        };
        let response: ResponseObject = self
            .bytes
            .create_json(&format!("{key}/manifest.json"), &response)
            .await?;
        self.read(&response.body).await?;
        Ok(response)
    }

    pub async fn transform<T: Serialize>(
        &self,
        prefix: &str,
        value: &T,
    ) -> Result<ObjectRef, WorkflowError> {
        self.bytes.transform(prefix, value).await
    }

    pub(crate) async fn remove(&self, key: &str) -> Result<(), WorkflowError> {
        self.bytes.remove(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::fs;

    #[tokio::test]
    async fn concurrent_captures_return_the_same_retained_response() {
        use ::object_store::throttle::{ThrottleConfig, ThrottledStore};
        use std::{sync::Arc, time::Duration};
        let (_, backend) = crate::test_support::object_storage(ObjectOwner::Strava);
        // Yield on publication so both calls observe a missing capture first.
        let backend = ThrottledStore::new(
            backend,
            ThrottleConfig {
                wait_put_per_call: Duration::from_millis(1),
                ..Default::default()
            },
        );
        let bytes = ByteStore::new(Arc::new(backend), ObjectOwner::Strava);
        let store = ObjectStore { bytes };
        let key = "strava/7/raw/event-1/activity";
        let (first, second) = tokio::join!(
            store.save_response(key, 200, Default::default(), b"first response"),
            store.save_response(key, 201, Default::default(), b"second response"),
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(first.body, second.body);
        assert_eq!(first.status, second.status);
        assert_eq!(first.observed_at, second.observed_at);
        assert_eq!(store.response(key).await.unwrap().unwrap().body, first.body);
    }
    #[tokio::test]
    async fn retains_exact_bytes_and_blocks_corrupt_sources() {
        let dir = tempfile::tempdir().unwrap();
        let store = ObjectStore::new(dir.path());
        let raw = br#"{ "id":20424619693, "unknown_vendor_field": true }"#;
        let saved = store
            .save_response(
                "strava/35999641/20424619693/raw/event-1/activity",
                200,
                Default::default(),
                raw,
            )
            .await
            .unwrap();
        assert_eq!(store.read(&saved.body).await.unwrap(), raw);
        assert_eq!(
            store
                .response("strava/35999641/20424619693/raw/event-1/activity")
                .await
                .unwrap()
                .unwrap()
                .body
                .sha256,
            saved.body.sha256
        );
        fs::remove_file(
            dir.path()
                .join("strava/35999641/20424619693/raw/event-1/activity/manifest.json"),
        )
        .await
        .unwrap();
        assert_eq!(
            store
                .response("strava/35999641/20424619693/raw/event-1/activity")
                .await
                .unwrap()
                .unwrap()
                .body
                .sha256,
            saved.body.sha256,
            "A saved capture recovers a missing checkpoint without downloading"
        );
        fs::write(dir.path().join(saved.body.path.clone()), b"corrupt")
            .await
            .unwrap();
        assert!(store
            .response("strava/35999641/20424619693/raw/event-1/activity")
            .await
            .is_err());
        assert!(store.write("../escape", raw).await.is_err());
    }
}
