use crate::activity_sport::ActivitySport;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Clone, Debug, Default, Deserialize, IntoParams, Serialize, ToSchema)]
pub struct HeatmapQuery {
    pub sport: Option<ActivitySport>,
    /// Activity start, inclusive, as an RFC3339 instant.
    pub from: Option<DateTime<Utc>>,
    /// Activity start, exclusive, as an RFC3339 instant.
    pub to: Option<DateTime<Utc>>,
    /// Required on tile and zone requests; use the revision returned by metadata.
    pub revision: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HeatmapMetadata {
    pub revision: String,
    pub filters: HeatmapQuery,
    pub ready: i64,
    pub pending: i64,
    pub failed: i64,
    pub skipped: i64,
    /// [west, south, east, north] for ready routes, or null when there are none.
    pub bounds: Option<[f64; 4]>,
    pub preparing: bool,
    pub tile_size: u32,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub style_version: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HeatmapZonePoint {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct HeatmapZones {
    pub revision: String,
    pub zones: Vec<HeatmapZonePoint>,
}

#[derive(Debug, thiserror::Error)]
pub enum HeatmapError {
    #[error("Heatmaps are unavailable on this site")]
    Disabled,
    #[error("{0}")]
    Invalid(String),
    #[error("Heatmap data changed; refresh metadata")]
    Stale,
    #[error("Heatmap is busy; retry shortly")]
    Busy,
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error("Heatmap rendering failed")]
    Render,
}

impl HeatmapQuery {
    pub fn validate(&self) -> Result<(), HeatmapError> {
        if matches!((self.from,self.to), (Some(from),Some(to)) if from >= to) {
            return Err(HeatmapError::Invalid("from must precede to".into()));
        }
        Ok(())
    }

    pub fn cache_key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.sport.map(|s| s.as_str()).unwrap_or("all"),
            self.from.map(|d| d.to_rfc3339()).unwrap_or_default(),
            self.to.map(|d| d.to_rfc3339()).unwrap_or_default()
        )
    }
}
