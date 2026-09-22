use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

const STORAGE_FORMAT_VERSION: u8 = 2;
const STORAGE_COORDINATE_SCALE: f64 = 10_000_000.0;
const STORAGE_DISTANCE_SCALE: f64 = 10.0;
const STORAGE_ELEVATION_SCALE: f64 = 10.0;
const STORAGE_SPEED_SCALE: f64 = 100.0;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityDerivedData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityLap {
    pub lap_index: i32,
    pub title: String,
    pub start_offset_seconds: Option<i32>,
    pub duration_seconds: Option<i32>,
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityChartPoint {
    pub elapsed_seconds: i32,
    pub distance_meters: Option<f64>,
    pub elevation_meters: Option<f64>,
    pub speed_mps: Option<f64>,
    pub heart_rate_bpm: Option<i32>,
    pub cadence_rpm: Option<i32>,
    pub power_watts: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityRoutePoint {
    pub elapsed_seconds: i32,
    pub latitude: f64,
    pub longitude: f64,
    pub distance_meters: Option<f64>,
    pub elevation_meters: Option<f64>,
    pub speed_mps: Option<f64>,
    pub heart_rate_bpm: Option<i32>,
    pub cadence_rpm: Option<i32>,
    pub power_watts: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, FromJsonQueryResult)]
pub struct StoredActivityDerivedData {
    v: u8,
    laps: Vec<ActivityLap>,
    chart_points: Vec<ActivityChartPoint>,
    route_points: Vec<ActivityRoutePoint>,
}

impl Default for StoredActivityDerivedData {
    fn default() -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: Vec::new(),
            chart_points: Vec::new(),
            route_points: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredActivityDerivedDataV2 {
    v: u8,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityDerivedDataV1 {
    v: u8,
    #[serde(default, rename = "l")]
    laps: Vec<CompactStoredActivityLapV1>,
    #[serde(default, rename = "c")]
    chart_points: Vec<CompactStoredActivityChartPointV1>,
    #[serde(default, rename = "r")]
    route_points: Vec<CompactStoredActivityRoutePointV1>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityLapV1(
    i32,
    String,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityChartPointV1(
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct CompactStoredActivityRoutePointV1(
    i32,
    i32,
    i32,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

#[derive(Debug, Clone, Default, PartialEq, FromJsonQueryResult)]
pub struct StoredRoutePointSeries(pub Vec<ActivityRoutePoint>);

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
enum StoredRoutePointSeriesRepr {
    V2(Vec<ActivityRoutePoint>),
    V1(Vec<CompactStoredActivityRoutePointV1>),
}

impl Serialize for StoredActivityDerivedData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        StoredActivityDerivedDataV2 {
            v: STORAGE_FORMAT_VERSION,
            laps: self.laps.clone(),
            chart_points: self.chart_points.clone(),
            route_points: self.route_points.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StoredActivityDerivedData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let is_schemaful = value.get("laps").is_some()
            || value.get("chart_points").is_some()
            || value.get("route_points").is_some();

        if is_schemaful {
            let value = StoredActivityDerivedDataV2::deserialize(value)
                .map_err(serde::de::Error::custom)?;
            if value.v != STORAGE_FORMAT_VERSION {
                tracing::warn!(
                    version = value.v,
                    "unsupported activity derived data format version"
                );
                return Ok(Self::default());
            }

            Ok(Self {
                v: STORAGE_FORMAT_VERSION,
                laps: value.laps,
                chart_points: value.chart_points,
                route_points: value.route_points,
            })
        } else {
            let value = CompactStoredActivityDerivedDataV1::deserialize(value)
                .map_err(serde::de::Error::custom)?;
            if value.v != 1 {
                tracing::warn!(
                    version = value.v,
                    "unsupported activity derived data format version"
                );
                return Ok(Self::default());
            }

            Ok(Self::from(value))
        }
    }
}

impl Serialize for StoredRoutePointSeries {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StoredRoutePointSeries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match StoredRoutePointSeriesRepr::deserialize(deserializer)? {
            StoredRoutePointSeriesRepr::V2(value) => Ok(Self(value)),
            StoredRoutePointSeriesRepr::V1(value) => Ok(Self(
                value.into_iter().map(ActivityRoutePoint::from).collect(),
            )),
        }
    }
}

impl From<&ActivityDerivedData> for StoredActivityDerivedData {
    fn from(value: &ActivityDerivedData) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: value.laps.clone(),
            chart_points: value.chart_points.clone(),
            route_points: value.route_points.clone(),
        }
    }
}

impl From<StoredActivityDerivedData> for ActivityDerivedData {
    fn from(value: StoredActivityDerivedData) -> Self {
        Self {
            laps: value.laps,
            chart_points: value.chart_points,
            route_points: value.route_points,
        }
    }
}

impl From<CompactStoredActivityDerivedDataV1> for StoredActivityDerivedData {
    fn from(value: CompactStoredActivityDerivedDataV1) -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            laps: value.laps.into_iter().map(ActivityLap::from).collect(),
            chart_points: value
                .chart_points
                .into_iter()
                .map(ActivityChartPoint::from)
                .collect(),
            route_points: value
                .route_points
                .into_iter()
                .map(ActivityRoutePoint::from)
                .collect(),
        }
    }
}

impl From<CompactStoredActivityLapV1> for ActivityLap {
    fn from(value: CompactStoredActivityLapV1) -> Self {
        Self {
            lap_index: value.0,
            title: value.1,
            start_offset_seconds: value.2,
            duration_seconds: value.3,
            distance_meters: decode_scaled_metric(value.4, STORAGE_DISTANCE_SCALE),
            elevation_gain_meters: decode_scaled_metric(value.5, STORAGE_ELEVATION_SCALE),
            elevation_loss_meters: decode_scaled_metric(value.6, STORAGE_ELEVATION_SCALE),
            average_speed_mps: decode_scaled_metric(value.7, STORAGE_SPEED_SCALE),
            max_speed_mps: decode_scaled_metric(value.8, STORAGE_SPEED_SCALE),
            average_heart_rate_bpm: value.9,
            max_heart_rate_bpm: value.10,
            average_cadence_rpm: value.11,
            max_cadence_rpm: value.12,
            calories: value.13,
        }
    }
}

impl From<CompactStoredActivityChartPointV1> for ActivityChartPoint {
    fn from(value: CompactStoredActivityChartPointV1) -> Self {
        Self {
            elapsed_seconds: value.0,
            distance_meters: decode_scaled_metric(value.1, STORAGE_DISTANCE_SCALE),
            elevation_meters: decode_scaled_metric(value.2, STORAGE_ELEVATION_SCALE),
            speed_mps: decode_scaled_metric(value.3, STORAGE_SPEED_SCALE),
            heart_rate_bpm: value.4,
            cadence_rpm: value.5,
            power_watts: None,
        }
    }
}

impl From<CompactStoredActivityRoutePointV1> for ActivityRoutePoint {
    fn from(value: CompactStoredActivityRoutePointV1) -> Self {
        Self {
            elapsed_seconds: value.0,
            latitude: decode_coordinate(value.1),
            longitude: decode_coordinate(value.2),
            distance_meters: decode_scaled_metric(value.3, STORAGE_DISTANCE_SCALE),
            elevation_meters: decode_scaled_metric(value.4, STORAGE_ELEVATION_SCALE),
            speed_mps: decode_scaled_metric(value.5, STORAGE_SPEED_SCALE),
            heart_rate_bpm: value.6,
            cadence_rpm: value.7,
            power_watts: None,
        }
    }
}

pub fn serialize_derived_activity_data(data: &ActivityDerivedData) -> StoredActivityDerivedData {
    StoredActivityDerivedData::from(data)
}

pub fn deserialize_derived_activity_data(
    raw: Option<&StoredActivityDerivedData>,
) -> ActivityDerivedData {
    raw.cloned()
        .map(ActivityDerivedData::from)
        .unwrap_or_default()
}

pub fn serialize_route_point_series(route_points: &[ActivityRoutePoint]) -> StoredRoutePointSeries {
    StoredRoutePointSeries(route_points.to_vec())
}

pub fn deserialize_route_point_series(
    raw: Option<&StoredRoutePointSeries>,
) -> Vec<ActivityRoutePoint> {
    raw.map(|series| series.0.clone()).unwrap_or_default()
}

fn decode_coordinate(value: i32) -> f64 {
    f64::from(value) / STORAGE_COORDINATE_SCALE
}

fn decode_scaled_metric(value: Option<i32>, scale: f64) -> Option<f64> {
    value.map(|metric| f64::from(metric) / scale)
}
