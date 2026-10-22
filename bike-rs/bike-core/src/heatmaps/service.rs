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
    #[tracing::instrument(name = "heatmap.metadata", skip_all, fields(bike.operation = "heatmap.metadata", user_id))]
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

    #[tracing::instrument(name = "heatmap.zones", skip_all, fields(bike.operation = "heatmap.zones", user_id))]
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
                HeatmapZonePoint { longitude, latitude }
            })
            .collect();
        Ok(HeatmapZones {
            revision: revision.to_string(),
            zones,
        })
    }

    #[tracing::instrument(name = "heatmap.tile", skip_all, fields(bike.operation = "heatmap.tile", user_id, tile.z = tile.z, tile.x = tile.x, tile.y = tile.y, cache.hit = tracing::field::Empty))]
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
            tracing::Span::current().record("cache.hit", true);
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
            tracing::Span::current().record("cache.hit", true);
            return Ok(image);
        }
        tracing::Span::current().record("cache.hit", false);
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| HeatmapError::Busy)?;
        let db = db.clone();
        let runtime = tokio::runtime::Handle::current();
        let image = tokio::task::spawn_blocking(crate::observability::blocking_with_current_trace(move || {
            render(&runtime, &db, request, permit)
        }))
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
        while cache.bytes + image.png.len() > CACHE_BYTES || cache.tiles.len() == cache.tiles.cap().get() {
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
    async fn check_revision(db: &DatabaseConnection, user_id: i32, revision: i64) -> Result<(), HeatmapError> {
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

#[tracing::instrument(name = "heatmap.tile.render", skip_all, fields(bike.operation = "heatmap.tile.render", user_id = request.user_id, chunk_count = tracing::field::Empty))]
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
    let mut chunks = 0_i64;
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
            chunks += 1;
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
    tracing::Span::current().record("chunk_count", chunks);
    // A full final generation check is also made by tile() before responding.
    runtime.block_on(HeatmapService::check_revision(db, request.user_id, request.revision))?;
    let png = raster.png().map_err(|_| HeatmapError::Render)?;
    let etag = format!("\"{:x}\"", Sha256::digest(&png));
    Ok(TileImage { png: png.into(), etag })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{activity_sport::ActivitySport, test_support};
    use sea_orm::Value;
    use std::collections::BTreeMap;

    type Row = BTreeMap<&'static str, Value>;

    fn enabled(value: bool) -> Vec<Row> {
        vec![BTreeMap::from([("enabled", value.into())])]
    }

    fn revision(value: i64) -> Vec<Row> {
        vec![BTreeMap::from([("revision", value.into())])]
    }

    fn query() -> HeatmapQuery {
        HeatmapQuery {
            revision: Some("1".into()),
            ..Default::default()
        }
    }

    fn tile() -> Tile {
        Tile { z: 0, x: 0, y: 0 }
    }

    fn rendered_tile() -> Vec<Vec<Row>> {
        vec![
            enabled(true),
            revision(1),
            vec![],
            revision(1),
            enabled(true),
            revision(1),
        ]
    }

    fn cached_tile() -> Vec<Vec<Row>> {
        vec![enabled(true), revision(1), enabled(true), revision(1)]
    }

    fn database(results: Vec<Vec<Row>>) -> DatabaseConnection {
        test_support::postgres_mock()
            .append_query_results(results)
            .into_connection()
    }

    #[tokio::test]
    async fn disabled_heatmaps_reject_metadata_zones_and_tiles() {
        let db = database(vec![enabled(false), enabled(false), enabled(false)]);
        let service = HeatmapService::default();
        assert!(matches!(
            service.metadata(&db, 7, query()).await,
            Err(HeatmapError::Disabled)
        ));
        assert!(matches!(
            service.zones(&db, 7, query()).await,
            Err(HeatmapError::Disabled)
        ));
        assert!(matches!(
            service.tile(&db, 7, query(), tile()).await,
            Err(HeatmapError::Disabled)
        ));
        assert_eq!(db.into_transaction_log().len(), 3);
    }

    #[tokio::test]
    async fn stale_revisions_reject_zones_and_tiles_before_loading_geometry() {
        let db = database(vec![enabled(true), revision(2), enabled(true), revision(2)]);
        let service = HeatmapService::default();
        assert!(matches!(service.zones(&db, 7, query()).await, Err(HeatmapError::Stale)));
        assert!(matches!(
            service.tile(&db, 7, query(), tile()).await,
            Err(HeatmapError::Stale)
        ));
        let log = format!("{:?}", db.into_transaction_log());
        assert!(!log.contains("heatmap_chunks"));
    }

    #[tokio::test]
    async fn cached_tiles_recheck_disable_and_revision_changes_before_returning() {
        for disabled in [false, true] {
            let mut results = rendered_tile();
            results.extend([enabled(true), revision(1), enabled(!disabled)]);
            if !disabled {
                results.push(revision(2));
            }
            let db = database(results);
            let service = HeatmapService::default();
            service.tile(&db, 7, query(), tile()).await.unwrap();
            let result = service.tile(&db, 7, query(), tile()).await;
            if disabled {
                assert!(matches!(result, Err(HeatmapError::Disabled)));
            } else {
                assert!(matches!(result, Err(HeatmapError::Stale)));
            }
            let reads = db
                .into_transaction_log()
                .iter()
                .flat_map(|transaction| transaction.statements())
                .filter(|statement| statement.sql.contains("JOIN \"heatmap_chunks\""))
                .count();
            assert_eq!(reads, 1);
        }
    }

    #[tokio::test]
    async fn cache_entries_are_shared_only_for_the_same_user_and_filters() {
        let mut results = rendered_tile();
        results.extend(cached_tile());
        results.extend(rendered_tile());
        results.extend(rendered_tile());
        let db = database(results);
        let service = HeatmapService::default();
        let first = service.tile(&db, 7, query(), tile()).await.unwrap();
        let cached = service.tile(&db, 7, query(), tile()).await.unwrap();
        let other_user = service.tile(&db, 8, query(), tile()).await.unwrap();
        let filtered = service
            .tile(
                &db,
                7,
                HeatmapQuery {
                    sport: Some(ActivitySport::RoadRide),
                    ..query()
                },
                tile(),
            )
            .await
            .unwrap();
        assert!(Arc::ptr_eq(&first.png, &cached.png));
        assert!(!Arc::ptr_eq(&first.png, &other_user.png));
        assert!(!Arc::ptr_eq(&first.png, &filtered.png));
        assert_eq!(service.cache.lock().await.tiles.len(), 3);
    }

    fn chunk(index: i32, y: f64) -> Row {
        BTreeMap::from([
            ("activity_id", 42.into()),
            ("chunk_index", index.into()),
            ("points", geometry::encode(&[[0.2, y], [0.8, y]]).into()),
        ])
    }

    #[tokio::test]
    async fn tile_render_includes_the_final_chunk_after_a_full_page() {
        let db = database(vec![
            enabled(true),
            revision(1),
            (0..64).map(|index| chunk(index, 0.25)).collect(),
            vec![chunk(64, 0.75)],
            vec![],
            revision(1),
            enabled(true),
            revision(1),
        ]);
        let image = HeatmapService::default().tile(&db, 7, query(), tile()).await.unwrap();
        let mut reader = png::Decoder::new(image.png.as_ref()).read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size()];
        reader.next_frame(&mut rgba).unwrap();
        for y in [128, 384] {
            assert!(rgba[(y * TILE_SIZE + 256) * 4 + 3] > 0, "missing page at y={y}");
        }
        let log = format!("{:?}", db.into_transaction_log());
        assert!(log.contains("Int(Some(63))"));
        assert!(log.contains("Int(Some(64))"));
    }

    #[tokio::test]
    async fn empty_and_reversed_periods_reject_metadata_zones_and_tiles_before_reading_data() {
        for seconds in [0, -1] {
            let db = database(vec![enabled(true), enabled(true), enabled(true)]);
            let service = HeatmapService::default();
            let invalid = HeatmapQuery {
                from: Some(test_support::timestamp()),
                to: Some(test_support::timestamp() + chrono::Duration::seconds(seconds)),
                ..query()
            };
            assert!(matches!(
                service.metadata(&db, 7, invalid.clone()).await,
                Err(HeatmapError::Invalid(_))
            ));
            assert!(matches!(
                service.zones(&db, 7, invalid.clone()).await,
                Err(HeatmapError::Invalid(_))
            ));
            assert!(matches!(
                service.tile(&db, 7, invalid, tile()).await,
                Err(HeatmapError::Invalid(_))
            ));
            assert_eq!(db.into_transaction_log().len(), 3);
        }
    }
}
