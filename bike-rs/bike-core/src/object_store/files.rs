//! Local files are transport/staging only; retained bytes always use the backend.
use super::{storage_error, ObjectStore};
use crate::workflow_error::WorkflowError;
use ::object_store::{ObjectStoreExt, WriteMultipart};
use futures::TryStreamExt;
use std::path::Path;
use tempfile::NamedTempFile;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

impl ObjectStore {
    pub(crate) async fn temporary_file(&self) -> Result<NamedTempFile, WorkflowError> {
        self.backend.temporary_file().await
    }

    pub async fn contains(&self, key: &str) -> Result<bool, WorkflowError> {
        let key = self.key(key)?;
        match self.backend.storage().await?.head(&key).await {
            Ok(_) => Ok(true),
            Err(::object_store::Error::NotFound { .. }) => Ok(false),
            Err(error) => Err(storage_error(error)),
        }
    }

    /// Stream large local/downloaded inputs into storage with bounded upload memory.
    pub async fn retain_file(&self, key: &str, path: &Path) -> Result<(), WorkflowError> {
        let key = self.key(key)?;
        let mut file = tokio::fs::File::open(path).await?;
        let upload = self
            .backend
            .storage()
            .await?
            .put_multipart(&key)
            .await
            .map_err(storage_error)?;
        let mut upload = WriteMultipart::new(upload);
        let mut buffer = vec![0; 1024 * 1024];
        let copied = async {
            loop {
                let length = file.read(&mut buffer).await?;
                if length == 0 {
                    return Ok::<_, WorkflowError>(());
                }
                upload.wait_for_capacity(2).await.map_err(storage_error)?;
                upload.write(&buffer[..length]);
            }
        }
        .await;
        if let Err(error) = copied {
            upload.abort().await.map_err(storage_error)?;
            return Err(error);
        }
        upload.finish().await.map_err(storage_error)?;
        Ok(())
    }

    /// ZIP parsers need seeking. Stream to disposable scratch, never to a retained path.
    pub async fn materialize(&self, key: &str) -> Result<NamedTempFile, WorkflowError> {
        let key = self.key(key)?;
        let result = self.backend.storage().await?.get(&key).await.map_err(storage_error)?;
        let temporary = self.temporary_file().await?;
        let mut file = tokio::fs::File::from_std(temporary.reopen()?);
        let mut chunks = result.into_stream();
        while let Some(chunk) = chunks.try_next().await.map_err(storage_error)? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(temporary)
    }
}
