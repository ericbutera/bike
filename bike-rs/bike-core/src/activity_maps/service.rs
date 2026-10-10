use super::{
    cache::Cache,
    client::SnapshotClient,
    metrics::METRICS,
    types::{MapImage, MapImageError, MapOptions, RenderRequest, Variant, STYLE_VERSION},
};
use crate::{activity_details::deserialize_derived_activity_data, entities::activities};
use sea_orm::DatabaseConnection;
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
    time::Duration,
};
use tokio::sync::Mutex;

pub struct MapImageService {
    cache: Arc<Cache>,
    client: SnapshotClient,
    pending: Mutex<()>,
}

impl MapImageService {
    pub async fn new(
        renderer_url: String,
        token: String,
        directory: PathBuf,
        ttl: Duration,
    ) -> Result<Self, MapImageError> {
        Ok(Self {
            cache: Cache::new(directory, ttl).await?,
            client: SnapshotClient::new(renderer_url, token)?,
            pending: Mutex::new(()),
        })
    }

    #[tracing::instrument(skip(self, db), fields(map.variant = ?variant, map.theme = ?options.theme))]
    pub async fn image(
        &self,
        db: &DatabaseConnection,
        user_id: i32,
        activity_id: i32,
        variant: Variant,
        style_version: &str,
        options: MapOptions,
    ) -> Result<MapImage, MapImageError> {
        if activity_id <= 0 || style_version != STYLE_VERSION || !matches!(options.dpr, 1 | 2) {
            return Err(MapImageError::Invalid);
        }
        // Ownership is checked before every cache read, including conditional requests.
        let activity = activities::Model::find_owned(db, activity_id, user_id)
            .await?
            .ok_or(MapImageError::NotFound)?;
        let route =
            deserialize_derived_activity_data(activity.derived_data_json.as_ref()).route_points;
        let request = RenderRequest::from_route(route, variant, options)?;
        self.cached_image(&request).await
    }

    async fn cached_image(&self, request: &RenderRequest) -> Result<MapImage, MapImageError> {
        let key = request.cache_key();
        if let Some(png) = self.cache.read(key.clone()).await? {
            return Ok(cached(png));
        }
        let _guard = self.pending.lock().await;
        if let Some(png) = self.cache.read(key.clone()).await? {
            return Ok(cached(png));
        }
        METRICS.misses.inc();
        let timer = METRICS.duration.with_label_values(&["miss"]).start_timer();
        let png = self.client.snapshot(request).await?;
        self.store_image(key, &png).await;
        timer.observe_duration();
        Ok(MapImage::new(png, false))
    }

    async fn store_image(&self, key: String, png: &[u8]) {
        if let Err(error) = self.cache.write(key, png.to_vec()).await {
            METRICS.write_failures.inc();
            tracing::error!(%error, stage = "cache_write", "Map image cache write failed");
        }
        if let Err(error) = self.prune().await {
            tracing::error!(%error, stage = "cache_stats", "Map cache statistics refresh failed");
        }
    }

    pub async fn prune(&self) -> Result<(), MapImageError> {
        let stats = self.cache.prune().await?;
        METRICS.pruned.inc_by(stats.pruned);
        METRICS.bytes.set(stats.bytes.min(i64::MAX as u64) as i64);
        METRICS.images.set(stats.images.min(i64::MAX as u64) as i64);
        Ok(())
    }

    pub fn maintain(self: &Arc<Self>) {
        let service = Arc::downgrade(self);
        tokio::spawn(maintain(service));
    }
}

fn cached(png: Vec<u8>) -> MapImage {
    METRICS.hits.inc();
    MapImage::new(png, true)
}

async fn maintain(service: Weak<MapImageService>) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));
    loop {
        interval.tick().await;
        let Some(service) = service.upgrade() else {
            return;
        };
        if let Err(error) = service.prune().await {
            tracing::error!(%error, stage = "cache_cleanup", "Map cache cleanup failed");
        }
    }
}
