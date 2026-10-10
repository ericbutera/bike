use crate::activity_details::ActivityRoutePoint;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use utoipa::{IntoParams, ToSchema};

pub const STYLE_VERSION: &str = "1";
const RENDER_REVISION: u8 = 3;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Variant {
    Thumbnail,
    Full,
}

#[derive(Debug, Clone, Copy, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
pub struct MapOptions {
    pub theme: Theme,
    #[param(minimum = 1, maximum = 2)]
    pub dpr: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(super) struct Point {
    pub latitude: f64,
    pub longitude: f64,
}

impl Point {
    fn valid(&self) -> bool {
        self.latitude.is_finite()
            && self.longitude.is_finite()
            && self.latitude.abs() <= 90.0
            && self.longitude.abs() <= 180.0
    }

    fn cache_json(&self) -> String {
        format!(
            "{{\"latitude\":{},\"longitude\":{}}}",
            js_number(self.latitude),
            js_number(self.longitude)
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RenderRequest {
    pub theme: Theme,
    pub variant: Variant,
    pub dpr: u8,
    pub points: Vec<Point>,
}

impl RenderRequest {
    pub fn from_route(
        route: Vec<ActivityRoutePoint>,
        variant: Variant,
        options: MapOptions,
    ) -> Result<Self, MapImageError> {
        let points: Vec<_> = route
            .into_iter()
            .map(|p| Point {
                latitude: p.latitude,
                longitude: p.longitude,
            })
            .filter(Point::valid)
            .collect();
        if points.len() < 2 {
            return Err(MapImageError::NotFound);
        }
        if points.len() > 100_000 {
            return Err(MapImageError::Invalid);
        }
        Ok(Self {
            theme: options.theme,
            variant,
            dpr: options.dpr,
            points,
        })
    }

    pub fn cache_key(&self) -> String {
        // Keep the deployed renderer's field order, revision, and JS number encoding.
        let theme = serde_json::to_string(&self.theme).expect("serialize map theme");
        let variant = serde_json::to_string(&self.variant).expect("serialize map variant");
        let points = self
            .points
            .iter()
            .map(Point::cache_json)
            .collect::<Vec<_>>()
            .join(",");
        let data = format!("{{\"revision\":{RENDER_REVISION},\"theme\":{theme},\"variant\":{variant},\"dpr\":{},\"points\":[{points}]}}", self.dpr);
        hex::encode(Sha256::digest(data.as_bytes()))
    }
}

fn js_number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value.abs() >= 1e-6 {
        return value.to_string();
    }
    format!("{value:e}")
}

#[derive(Debug, thiserror::Error)]
pub enum MapImageError {
    #[error("Invalid map image request")]
    Invalid,
    #[error("Activity map not found")]
    NotFound,
    #[error(transparent)]
    Database(#[from] sea_orm::DbErr),
    #[error(transparent)]
    Cache(#[from] std::io::Error),
    #[error(transparent)]
    Endpoint(#[from] tonic::transport::Error),
    #[error(transparent)]
    Snapshot(Box<tonic::Status>),
    #[error("Invalid map snapshot worker credentials")]
    Credentials,
    #[error("Map image task failed: {0}")]
    Task(String),
    #[error("Map snapshot worker returned an invalid PNG")]
    InvalidPNG,
}

pub struct MapImage {
    pub png: Vec<u8>,
    pub etag: String,
    pub cache_hit: bool,
}

impl MapImage {
    pub(super) fn new(png: Vec<u8>, cache_hit: bool) -> Self {
        let etag = format!("\"{}\"", hex::encode(Sha256::digest(&png)));
        Self {
            png,
            etag,
            cache_hit,
        }
    }
}
