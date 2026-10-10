use super::types::MapImageError;
use std::{
    fs::{self, File, FileTimes, OpenOptions},
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};
use uuid::Uuid;

pub(super) struct Cache {
    pub root: PathBuf,
    pub ttl: Duration,
    operations: std::sync::Mutex<()>,
}
pub(super) struct Stats {
    pub bytes: u64,
    pub images: u64,
    pub pruned: u64,
}

impl Cache {
    pub async fn new(root: PathBuf, ttl: Duration) -> Result<Arc<Self>, MapImageError> {
        if ttl.is_zero() {
            return Err(MapImageError::Invalid);
        }
        tokio::fs::create_dir_all(&root).await?;
        Ok(Arc::new(Self {
            root,
            ttl,
            operations: std::sync::Mutex::new(()),
        }))
    }

    pub async fn read(self: &Arc<Self>, key: String) -> Result<Option<Vec<u8>>, MapImageError> {
        let cache = Arc::clone(self);
        tokio::task::spawn_blocking(move || cache.read_file(&key))
            .await
            .map_err(|e| MapImageError::Task(e.to_string()))?
            .map_err(Into::into)
    }

    fn read_file(&self, key: &str) -> io::Result<Option<Vec<u8>>> {
        let _guard = self
            .operations
            .lock()
            .map_err(|_| io::Error::other("Map cache lock poisoned"))?;
        let path = self.root.join(format!("{key}.png"));
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if self.expired(metadata.modified()?) {
            remove_missing_ok(&path)?;
            return Ok(None);
        }
        let image = fs::read(&path)?;
        File::open(&path)?.set_times(FileTimes::new().set_modified(SystemTime::now()))?;
        Ok(Some(image))
    }

    pub async fn write(self: &Arc<Self>, key: String, png: Vec<u8>) -> Result<(), MapImageError> {
        let cache = Arc::clone(self);
        tokio::task::spawn_blocking(move || cache.write_file(&key, &png))
            .await
            .map_err(|e| MapImageError::Task(e.to_string()))?
            .map_err(Into::into)
    }

    fn write_file(&self, key: &str, png: &[u8]) -> io::Result<()> {
        let _guard = self
            .operations
            .lock()
            .map_err(|_| io::Error::other("Map cache lock poisoned"))?;
        let destination = self.root.join(format!("{key}.png"));
        let temporary = self.root.join(format!("{key}.png.tmp-{}", Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temporary)?;
            file.write_all(png)?;
            drop(file);
            fs::rename(&temporary, destination)
        })();
        remove_missing_ok(&temporary)?;
        result
    }

    pub async fn prune(self: &Arc<Self>) -> Result<Stats, MapImageError> {
        let cache = Arc::clone(self);
        tokio::task::spawn_blocking(move || cache.prune_files())
            .await
            .map_err(|e| MapImageError::Task(e.to_string()))?
            .map_err(Into::into)
    }

    fn prune_files(&self) -> io::Result<Stats> {
        let _guard = self
            .operations
            .lock()
            .map_err(|_| io::Error::other("Map cache lock poisoned"))?;
        let mut stats = Stats {
            bytes: 0,
            images: 0,
            pruned: 0,
        };
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = match entry.metadata() {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            if !metadata.is_file() {
                continue;
            }
            let owned = owned_file(&name);
            if owned && self.expired(metadata.modified()?) {
                remove_missing_ok(&entry.path())?;
                stats.pruned += 1;
            } else {
                stats.bytes += metadata.len();
                stats.images += u64::from(owned && name.ends_with(".png"));
            }
        }
        Ok(stats)
    }

    fn expired(&self, modified: SystemTime) -> bool {
        SystemTime::now()
            .duration_since(modified)
            .unwrap_or_default()
            > self.ttl
    }
}

fn owned_file(name: &str) -> bool {
    let Some((key, suffix)) = name.split_once(".png") else {
        return false;
    };
    key.len() == 64
        && key.bytes().all(|b| b.is_ascii_hexdigit())
        && (suffix.is_empty() || suffix.starts_with(".tmp-"))
}

fn remove_missing_ok(path: &std::path::Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
