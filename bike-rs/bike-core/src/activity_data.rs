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
    #[serde(
        default,
        skip_serializing_if = "crate::activity_recording::RecordingContext::is_empty"
    )]
    pub recording: crate::activity_recording::RecordingContext,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub laps: Vec<ActivityLap>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chart_points: Vec<ActivityChartPoint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route_points: Vec<ActivityRoutePoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({"lap_index": 1,"title": "Warmup","start_offset_seconds": 0,"duration_seconds": 900,"distance_meters": 4200.5,"average_heart_rate_bpm": 132,"average_speed_mps": 4.67}))]
pub struct ActivityLap {
    #[schema(example = 1)]
    pub lap_index: i32,
    #[schema(example = "Warmup")]
    pub title: String,
    #[schema(example = 0)]
    pub start_offset_seconds: Option<i32>,
    #[schema(example = 900)]
    pub duration_seconds: Option<i32>,
    #[schema(example = 4200.5)]
    pub distance_meters: Option<f64>,
    pub elevation_gain_meters: Option<f64>,
    pub elevation_loss_meters: Option<f64>,
    #[schema(example = 4.67)]
    pub average_speed_mps: Option<f64>,
    pub max_speed_mps: Option<f64>,
    #[schema(example = 132)]
    pub average_heart_rate_bpm: Option<i32>,
    pub max_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<i32>,
    pub max_cadence_rpm: Option<i32>,
    pub calories: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({"elapsed_seconds": 900,"distance_meters": 4200.5,"heart_rate_bpm": 148,"power_watts": 235,"cadence_rpm": 82,"speed_mps": 6.4,"elevation_meters": 183.2}))]
pub struct ActivityChartPoint {
    #[schema(example = 900)]
    pub elapsed_seconds: i32,
    #[schema(example = 4200.5)]
    pub distance_meters: Option<f64>,
    #[schema(example = 183.2)]
    pub elevation_meters: Option<f64>,
    #[schema(example = 6.4)]
    pub speed_mps: Option<f64>,
    #[schema(example = 148)]
    pub heart_rate_bpm: Option<i32>,
    #[schema(example = 82)]
    pub cadence_rpm: Option<i32>,
    #[schema(example = 235)]
    pub power_watts: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({"elapsed_seconds": 900,"latitude": 42.3314,"longitude": -83.0458,"distance_meters": 4200.5,"elevation_meters": 183.2,"heart_rate_bpm": 148,"power_watts": 235,"cadence_rpm": 82,"speed_mps": 6.4}))]
pub struct ActivityRoutePoint {
    #[schema(example = 900)]
    pub elapsed_seconds: i32,
    #[schema(example = 42.3314)]
    pub latitude: f64,
    #[schema(example = -83.0458)]
    pub longitude: f64,
    #[schema(example = 4200.5)]
    pub distance_meters: Option<f64>,
    #[schema(example = 183.2)]
    pub elevation_meters: Option<f64>,
    #[schema(example = 6.4)]
    pub speed_mps: Option<f64>,
    #[schema(example = 148)]
    pub heart_rate_bpm: Option<i32>,
    #[schema(example = 82)]
    pub cadence_rpm: Option<i32>,
    #[schema(example = 235)]
    pub power_watts: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, FromJsonQueryResult)]
pub struct StoredActivityDerivedData {
    v: u8,
    recording: crate::activity_recording::RecordingContext,
    laps: Vec<ActivityLap>,
    chart_points: Vec<ActivityChartPoint>,
    route_points: Vec<ActivityRoutePoint>,
}

impl Default for StoredActivityDerivedData {
    fn default() -> Self {
        Self {
            v: STORAGE_FORMAT_VERSION,
            recording: Default::default(),
            laps: Vec::new(),
            chart_points: Vec::new(),
            route_points: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct StoredActivityDerivedDataV2 {
    v: u8,
    #[serde(
        default,
        skip_serializing_if = "crate::activity_recording::RecordingContext::is_empty"
    )]
    recording: crate::activity_recording::RecordingContext,
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
    #[serde(default)]
    recording: crate::activity_recording::RecordingContext,
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
            recording: self.recording.clone(),
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
        let is_schemaful = value.get("v").and_then(serde_json::Value::as_u64) == Some(2)
            || value.get("laps").is_some()
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
                recording: value.recording,
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
            recording: value.recording.clone(),
            laps: value.laps.clone(),
            chart_points: value.chart_points.clone(),
            route_points: value.route_points.clone(),
        }
    }
}

impl From<StoredActivityDerivedData> for ActivityDerivedData {
    fn from(value: StoredActivityDerivedData) -> Self {
        Self {
            recording: value.recording,
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
            recording: value.recording,
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

#[cfg(test)]
mod recording_tests {
    use super::*;
    use crate::activity_recording::{RecordingContext, RecordingEnvironment};

    #[test]
    fn recording_evidence_survives_storage_and_legacy_format_upgrade() {
        let mut context = RecordingContext::default();
        context.observe("fit.creator.manufacturer", "zwift");
        let derived = ActivityDerivedData {
            recording: context.clone(),
            ..Default::default()
        };
        let json = serde_json::to_value(serialize_derived_activity_data(&derived)).unwrap();
        let stored: StoredActivityDerivedData = serde_json::from_value(json).unwrap();
        assert_eq!(
            deserialize_derived_activity_data(Some(&stored)).recording,
            context
        );
        let legacy: StoredActivityDerivedData =
            serde_json::from_value(serde_json::json!({"v":1,"recording":context,"r":[]})).unwrap();
        let upgraded = serde_json::to_value(legacy).unwrap();
        assert_eq!(upgraded["v"], 2);
        assert_eq!(upgraded["recording"]["environment"], "virtual");
    }

    #[test]
    fn absent_recording_metadata_preserves_real_legacy_activities() {
        for json in [
            serde_json::json!({"v":1,"r":[]}),
            serde_json::json!({"v":2,"route_points":[]}),
        ] {
            let stored = serde_json::from_value(json).unwrap();
            let derived = deserialize_derived_activity_data(Some(&stored));
            assert_eq!(derived.recording.environment, RecordingEnvironment::Unknown);
            assert!(!derived.recording.excluded());
        }
    }
}
