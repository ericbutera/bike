use super::{
    data::HeatmapData,
    geometry,
    projection::Projection,
    raster::{Raster, Tile, TILE_SIZE},
    types::{HeatmapError, HeatmapMetadata, HeatmapQuery, HeatmapZonePoint, HeatmapZones},
    STYLE_VERSION,
};
use lru::LruCache;
use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::{Arc, Weak},
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

const CACHE_BYTES: usize = 64 * 1024 * 1024;
const RENDER_DEADLINE: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct TileImage {
    pub png: Arc<[u8]>,
    pub etag: String,
}

struct Cache {
    tiles: LruCache<String, TileImage>,
    bytes: usize,
}

pub struct HeatmapService {
    cache: Mutex<Cache>,
    locks: Mutex<HashMap<String, Weak<Mutex<()>>>>,
    permits: Arc<Semaphore>,
}

impl Default for HeatmapService {
    fn default() -> Self {
        Self {
            cache: Mutex::new(Cache {
                tiles: LruCache::new(NonZeroUsize::new(512).unwrap()),
                bytes: 0,
            }),
            locks: Mutex::new(HashMap::new()),
            permits: Arc::new(Semaphore::new(2)),
        }
    }
}

impl HeatmapService {
    pub async fn metadata(
        &self,
        db: &DatabaseConnection,
        user_id: i32,
        mut query: HeatmapQuery,
    ) -> Result<HeatmapMetadata, HeatmapError> {
        Self::check_enabled(db).await?;
        query.validate()?;
        query.revision = None;
        let p = HeatmapData::progress(db, user_id, &query).await?;
        let bounds = match (p.min_x, p.min_y, p.max_x, p.max_y) {
            (Some(x1), Some(y1), Some(x2), Some(y2)) => {
                let sw = geometry::unproject([x1, y2]);
                let ne = geometry::unproject([x2, y1]);
                Some([sw[0], sw[1], ne[0], ne[1]])
            }
            _ => None,
        };
        Ok(HeatmapMetadata {
            revision: p.revision.to_string(),
            filters: query,
            ready: p.ready,
            pending: p.pending,
            failed: p.failed,
            skipped: p.skipped,
            bounds,
            preparing: p.pending > 0,
            tile_size: TILE_SIZE as u32,
            min_zoom: 0,
            max_zoom: 18,
            style_version: STYLE_VERSION.into(),
        })
    }

    pub async fn zones(
        &self,
        db: &DatabaseConnection,
        user_id: i32,
        query: HeatmapQuery,
    ) -> Result<HeatmapZones, HeatmapError> {
        Self::check_enabled(db).await?;
        query.validate()?;
        let revision = query
            .revision
            .as_deref()
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0)
            .ok_or_else(|| HeatmapError::Invalid("Zone requests require a revision".into()))?;
        Self::check_revision(db, user_id, revision).await?;
        let points = HeatmapData::activity_centers(db, user_id, &query).await?;
        Self::check_revision(db, user_id, revision).await?;
        let zones = points
            .into_iter()
            .map(|point| {
                let [longitude, latitude] = geometry::unproject([point.x, point.y]);
                HeatmapZonePoint {
                    longitude,
                    latitude,
                }
            })
            .collect();
        Ok(HeatmapZones {
            revision: revision.to_string(),
            zones,
        })
    }

    pub async fn tile(
        &self,
        db: &DatabaseConnection,
        user_id: i32,
        query: HeatmapQuery,
        tile: Tile,
    ) -> Result<TileImage, HeatmapError> {
        Self::check_enabled(db).await?;
        query.validate()?;
        if !tile.is_valid() {
            return Err(HeatmapError::Invalid("Invalid tile coordinates".into()));
        }
        let revision = query
            .revision
            .as_deref()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v >= 0)
            .ok_or_else(|| HeatmapError::Invalid("Tile requests require a revision".into()))?;
        Self::check_revision(db, user_id, revision).await?;
        let key = format!(
            "{user_id}:{revision}:{}:{}:{}:{}:{STYLE_VERSION}",
            query.cache_key(),
            tile.z,
            tile.x,
            tile.y
        );
        let image = tokio::time::timeout(
            RENDER_DEADLINE,
            self.cached_tile(
                db,
                RenderRequest {
                    user_id,
                    revision,
                    query,
                    tile,
                },
                key,
            ),
        )
        .await
        .map_err(|_| HeatmapError::Busy)??;
        // Recheck after rendering or a cache wait: changed/deleted routes cannot
        // be returned under an obsolete revision, including conditional requests.
        Self::check_enabled(db).await?;
        Self::check_revision(db, user_id, revision).await?;
        Ok(image)
    }

    async fn cached_tile(
        &self,
        db: &DatabaseConnection,
        request: RenderRequest,
        key: String,
    ) -> Result<TileImage, HeatmapError> {
        if let Some(image) = self.cached(&key).await {
            return Ok(image);
        }
        let lock = {
            let mut locks = self.locks.lock().await;
            locks.retain(|_, v| v.strong_count() > 0);
            if let Some(lock) = locks.get(&key).and_then(Weak::upgrade) {
                lock
            } else {
                let lock = Arc::new(Mutex::new(()));
                locks.insert(key.clone(), Arc::downgrade(&lock));
                lock
            }
        };
        let _guard = lock.lock().await;
        if let Some(image) = self.cached(&key).await {
            return Ok(image);
        }
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| HeatmapError::Busy)?;
        let db = db.clone();
        let runtime = tokio::runtime::Handle::current();
        let image = tokio::task::spawn_blocking(move || render(&runtime, &db, request, permit))
            .await
            .map_err(|_| HeatmapError::Render)??;
        self.remember(key, image.clone()).await;
        Ok(image)
    }

    async fn cached(&self, key: &str) -> Option<TileImage> {
        self.cache.lock().await.tiles.get(key).cloned()
    }

    async fn remember(&self, key: String, image: TileImage) {
        let mut cache = self.cache.lock().await;
        if image.png.len() > CACHE_BYTES {
            return;
        }
        while cache.bytes + image.png.len() > CACHE_BYTES
            || cache.tiles.len() == cache.tiles.cap().get()
        {
            if let Some((_, old)) = cache.tiles.pop_lru() {
                cache.bytes -= old.png.len();
            } else {
                break;
            }
        }
        cache.bytes += image.png.len();
        if let Some(old) = cache.tiles.put(key, image) {
            cache.bytes -= old.png.len();
        }
    }

    async fn check_enabled(db: &DatabaseConnection) -> Result<(), HeatmapError> {
        if Projection::enabled(db).await? {
            Ok(())
        } else {
            Err(HeatmapError::Disabled)
        }
    }
    async fn check_revision(
        db: &DatabaseConnection,
        user_id: i32,
        revision: i64,
    ) -> Result<(), HeatmapError> {
        if HeatmapData::revision(db, user_id).await? == revision {
            Ok(())
        } else {
            Err(HeatmapError::Stale)
        }
    }
}

struct RenderRequest {
    user_id: i32,
    revision: i64,
    query: HeatmapQuery,
    tile: Tile,
}

fn render(
    runtime: &tokio::runtime::Handle,
    db: &DatabaseConnection,
    request: RenderRequest,
    _permit: OwnedSemaphorePermit,
) -> Result<TileImage, HeatmapError> {
    let deadline = Instant::now() + RENDER_DEADLINE;
    let mut raster = Raster::new(request.tile);
    let mut cursor = (0, -1);
    let mut activity = None;
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(HeatmapError::Busy)?;
        let page = runtime
            .block_on(tokio::time::timeout(
                remaining,
                HeatmapData::tile_page(db, request.user_id, &request.query, request.tile, cursor),
            ))
            .map_err(|_| HeatmapError::Busy)??;
        if page.is_empty() {
            break;
        }
        for chunk in page {
            if Instant::now() >= deadline {
                return Err(HeatmapError::Busy);
            }
            if activity != Some(chunk.activity_id) {
                raster.finish_activity();
                activity = Some(chunk.activity_id);
            }
            let points = geometry::decode(&chunk.points).ok_or(HeatmapError::Render)?;
            raster.add_chunk(&points);
            cursor = (chunk.activity_id, chunk.chunk_index);
        }
    }
    raster.finish_activity();
    // A full final generation check is also made by tile() before responding.
    runtime.block_on(HeatmapService::check_revision(
        db,
        request.user_id,
        request.revision,
    ))?;
    let png = raster.png().map_err(|_| HeatmapError::Render)?;
    let etag = format!("\"{:x}\"", Sha256::digest(&png));
    Ok(TileImage {
        png: png.into(),
        etag,
    })
}
