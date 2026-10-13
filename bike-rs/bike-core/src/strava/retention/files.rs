use super::{metrics, protection::Protection, RetentionResult};
use crate::{strava::objects::ObjectStore, workflow_error::WorkflowError};
use chrono::{DateTime, Utc};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
use tokio::fs;

pub(super) struct StoredFile {
    pub path: String,
    pub bytes: u64,
    pub modified: DateTime<Utc>,
}
impl StoredFile {
    pub fn expires(&self, protected: &Protection, cutoff: DateTime<Utc>) -> bool {
        self.modified < cutoff && !protected.contains(&self.path)
    }
    pub fn kind(&self, sources: &BTreeSet<String>) -> &'static str {
        if sources.contains(&self.path)
            || self.path.starts_with(crate::strava::paths::SOURCES)
            || Path::new(&self.path)
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("strava_"))
        {
            "source"
        } else if self.path.contains("/raw/") {
            "raw"
        } else if self.path.contains("/transforms/") {
            "transform"
        } else {
            "metadata"
        }
    }
}

pub(super) async fn inventory(
    root: &str,
    sources: &BTreeSet<String>,
) -> Result<Vec<StoredFile>, WorkflowError> {
    let store = ObjectStore::new(root);
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    // The legacy sweep only selects superseded native Strava payload filenames.
    // Unrelated uploads, originals and other providers are outside its scope.
    for namespace in ["strava", "activity-imports"] {
        for file in walk(Path::new(root), namespace).await? {
            if namespace == "strava" || legacy_payload(&file.path) {
                paths.insert(file.path.clone());
                files.push(file);
            }
        }
    }
    for source in sources.difference(&paths) {
        let path = store.path(source)?;
        let metadata = fs::symlink_metadata(path).await?;
        if !metadata.is_file() {
            return Err(WorkflowError::internal(
                "Strava rebuild source must be a regular file",
            ));
        }
        files.push(StoredFile {
            path: source.clone(),
            bytes: metadata.len(),
            modified: metadata.modified()?.into(),
        });
    }
    Ok(files)
}

fn legacy_payload(path: &str) -> bool {
    Path::new(path).file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.starts_with("strava_") && name.ends_with(".json")
    })
}

async fn walk(root: &Path, namespace: &str) -> Result<Vec<StoredFile>, WorkflowError> {
    let mut pending = vec![PathBuf::from(namespace)];
    let mut files = Vec::new();
    while let Some(relative) = pending.pop() {
        let path = root.join(&relative);
        let metadata = match fs::symlink_metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if metadata.is_dir() {
            let mut entries = fs::read_dir(path).await?;
            while let Some(entry) = entries.next_entry().await? {
                pending.push(relative.join(entry.file_name()));
            }
        } else if metadata.is_file() {
            files.push(StoredFile {
                path: relative.to_string_lossy().into_owned(),
                bytes: metadata.len(),
                modified: metadata.modified()?.into(),
            });
        } else {
            return Err(WorkflowError::internal(format!(
                "Refusing to follow non-regular storage entry {}",
                relative.display()
            )));
        }
    }
    Ok(files)
}

pub(super) async fn prune(
    store: &ObjectStore,
    inventory: &[StoredFile],
    protected: &Protection,
    cutoff: DateTime<Utc>,
) -> Result<RetentionResult, WorkflowError> {
    let mut result = RetentionResult::default();
    for file in inventory
        .iter()
        .filter(|file| file.expires(protected, cutoff))
    {
        let path = store.path(&file.path)?;
        match fs::remove_file(&path).await {
            Ok(()) => {
                result.files += 1;
                result.bytes += file.bytes;
                metrics::pruned(file.bytes);
                let namespace = store.path(file.path.split('/').next().unwrap_or("strava"))?;
                remove_empty_parents(path.parent(), &namespace).await?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(result)
}

async fn remove_empty_parents(
    mut directory: Option<&Path>,
    namespace: &Path,
) -> Result<(), WorkflowError> {
    while let Some(path) = directory {
        if path == namespace {
            return Ok(());
        }
        match fs::remove_dir(path).await {
            Ok(()) => directory = path.parent(),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
                ) =>
            {
                return Ok(())
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
