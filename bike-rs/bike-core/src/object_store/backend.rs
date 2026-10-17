//! The only filesystem setup for shared objects; the library owns file I/O.
use crate::workflow_error::WorkflowError;
use ::object_store::{local::LocalFileSystem, ObjectStore};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::OnceCell;

pub(super) enum Backend {
    Ready(Arc<dyn ObjectStore>),
    Filesystem {
        root: PathBuf,
        storage: OnceCell<LocalFileSystem>,
    },
}

impl Backend {
    pub(super) async fn temporary_file(&self) -> Result<tempfile::NamedTempFile, WorkflowError> {
        match self {
            Self::Ready(_) => Ok(tempfile::NamedTempFile::new()?),
            Self::Filesystem { root, .. } => {
                // Production has a read-only root filesystem. Keep disposable scratch
                // on the already writable storage mount, separate from domain keys.
                let scratch = root.join(".scratch");
                tokio::fs::create_dir_all(&scratch).await?;
                Ok(tempfile::NamedTempFile::new_in(scratch)?)
            }
        }
    }

    pub(super) fn filesystem(root: impl AsRef<Path>) -> Self {
        Self::Filesystem {
            root: root.as_ref().to_owned(),
            storage: OnceCell::new(),
        }
    }

    pub(super) async fn storage(&self) -> Result<&dyn ObjectStore, WorkflowError> {
        match self {
            Self::Ready(storage) => Ok(storage.as_ref()),
            Self::Filesystem { root, storage } => {
                let store = storage
                    .get_or_try_init(|| async {
                        tokio::fs::create_dir_all(root).await?;
                        LocalFileSystem::new_with_prefix(root)
                            .map(|store| store.with_fsync(true))
                            .map_err(|error| WorkflowError::internal(error.to_string()))
                    })
                    .await?;
                Ok(store)
            }
        }
    }
}
