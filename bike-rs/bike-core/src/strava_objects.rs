//! Immutable provider responses and transforms on Bike's existing upload volume.
use crate::workflow_error::WorkflowError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectRef {
    pub path: String,
    pub sha256: String,
    pub bytes: usize,
}

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
    root: PathBuf,
}

impl ObjectStore {
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            root: root.as_ref().to_owned(),
        }
    }

    pub async fn read(&self, object: &ObjectRef) -> Result<Vec<u8>, WorkflowError> {
        let path = self.path(&object.path)?;
        let bytes = fs::read(path).await.map_err(|error| {
            WorkflowError::internal(format!(
                "Saved Strava object {} is unavailable: {error}",
                object.path
            ))
        })?;
        if checksum(&bytes) != object.sha256 || bytes.len() != object.bytes {
            return Err(WorkflowError::internal(format!(
                "Saved Strava object {} failed checksum verification",
                object.path
            )));
        }
        Ok(bytes)
    }

    pub async fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        object: &ObjectRef,
    ) -> Result<T, WorkflowError> {
        serde_json::from_slice(&self.read(object).await?).map_err(json_error)
    }

    pub async fn write(&self, prefix: &str, bytes: &[u8]) -> Result<ObjectRef, WorkflowError> {
        let hash = checksum(bytes);
        let relative = format!("{prefix}/{hash}.json");
        let path = self.path(&relative)?;
        let object = ObjectRef {
            path: relative,
            sha256: hash,
            bytes: bytes.len(),
        };
        if fs::try_exists(&path).await.map_err(io_error)? {
            self.read(&object).await?;
            return Ok(object);
        }
        publish(&path, bytes).await?;
        Ok(object)
    }

    pub async fn reference(&self, key: &str) -> Result<Option<ObjectRef>, WorkflowError> {
        let bytes = match fs::read(self.path(&format!("{key}/reference.json"))?).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(error)),
        };
        let reference: ObjectRef = serde_json::from_slice(&bytes).map_err(json_error)?;
        self.read(&reference).await?;
        Ok(Some(reference))
    }

    pub async fn save_reference(
        &self,
        key: &str,
        reference: &ObjectRef,
    ) -> Result<(), WorkflowError> {
        self.read(reference).await?;
        publish(
            &self.path(&format!("{key}/reference.json"))?,
            &serde_json::to_vec(reference).map_err(json_error)?,
        )
        .await?;
        let saved = self
            .reference(key)
            .await?
            .ok_or_else(|| WorkflowError::internal("Strava reference was not retained"))?;
        if saved.sha256 != reference.sha256 {
            return Err(WorkflowError::conflict(
                "Saved Strava source reference is immutable",
            ));
        }
        Ok(())
    }

    pub async fn response(&self, key: &str) -> Result<Option<ResponseObject>, WorkflowError> {
        let path = self.path(&format!("{key}/manifest.json"))?;
        let bytes = match fs::read(path).await {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let capture = match fs::read(self.path(&format!("{key}/capture.json"))?).await {
                    Ok(bytes) => serde_json::from_slice(&bytes).map_err(json_error)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(error) => return Err(io_error(error)),
                };
                return self.materialize(key, capture).await.map(Some);
            }
            Err(error) => return Err(io_error(error)),
        };
        let response: ResponseObject = serde_json::from_slice(&bytes).map_err(json_error)?;
        self.read(&response.body).await?;
        Ok(Some(response))
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
        let encrypted = crate::strava_credentials::seal(bytes)?;
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
        publish(
            &self.path(&format!("{key}/capture.json"))?,
            &serde_json::to_vec(&capture).map_err(json_error)?,
        )
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
            crate::strava_credentials::open(&ciphertext).map(String::into_bytes)
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
        publish(
            &self.path(&format!("{key}/manifest.json"))?,
            &serde_json::to_vec(&response).map_err(json_error)?,
        )
        .await?;
        Ok(response)
    }

    pub async fn transform<T: Serialize>(
        &self,
        prefix: &str,
        value: &T,
    ) -> Result<ObjectRef, WorkflowError> {
        self.write(prefix, &serde_json::to_vec(value).map_err(json_error)?)
            .await
    }

    fn path(&self, relative: &str) -> Result<PathBuf, WorkflowError> {
        if relative.is_empty()
            || Path::new(relative)
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err(WorkflowError::bad_request("Invalid Strava object path"));
        }
        Ok(self.root.join(relative))
    }
}

pub fn checksum(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

async fn publish(path: &Path, bytes: &[u8]) -> Result<(), WorkflowError> {
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| WorkflowError::internal("Missing object directory"))?,
    )
    .await
    .map_err(io_error)?;
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = async {
        use tokio::io::AsyncWriteExt;
        let mut file = fs::File::create(&temporary).await.map_err(io_error)?;
        file.write_all(bytes).await.map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        // Linking publishes a complete file without replacing an existing object.
        match fs::hard_link(&temporary, path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
            Err(error) => Err(io_error(error)),
        }
    }
    .await;
    let cleanup = fs::remove_file(&temporary).await;
    result?;
    cleanup.map_err(io_error)?;
    fs::File::open(
        path.parent()
            .ok_or_else(|| WorkflowError::internal("Missing object directory"))?,
    )
    .await
    .map_err(io_error)?
    .sync_all()
    .await
    .map_err(io_error)?;
    Ok(())
}

fn io_error(error: std::io::Error) -> WorkflowError {
    WorkflowError::internal(error.to_string())
}
fn json_error(error: serde_json::Error) -> WorkflowError {
    WorkflowError::internal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
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
