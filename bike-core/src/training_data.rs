use sea_orm::FromJsonQueryResult;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const HEART_RATE_ZONE_LABELS: [&str; 5] = ["Z1", "Z2", "Z3", "Z4", "Z5"];
const HEART_RATE_ZONE_SHARE_PERCENT_SCALE: f64 = 1000.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ActivityHeartRateZoneSummary {
    pub zone: i32,
    pub label: String,
    pub min_bpm: Option<i32>,
    pub max_bpm: Option<i32>,
    pub duration_seconds: i32,
    pub share_percent: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct StoredHeartRateZoneBounds(pub Vec<i32>);

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, FromJsonQueryResult)]
pub struct StoredActivityHeartRateZones(pub Vec<StoredActivityHeartRateZoneSummary>);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredActivityHeartRateZoneSummary(
    pub i32,
    pub Option<i32>,
    pub Option<i32>,
    pub i32,
    pub i32,
);

impl From<&ActivityHeartRateZoneSummary> for StoredActivityHeartRateZoneSummary {
    fn from(value: &ActivityHeartRateZoneSummary) -> Self {
        Self(
            value.zone,
            value.min_bpm,
            value.max_bpm,
            value.duration_seconds,
            encode_share_percent(value.share_percent),
        )
    }
}

impl From<StoredActivityHeartRateZoneSummary> for ActivityHeartRateZoneSummary {
    fn from(value: StoredActivityHeartRateZoneSummary) -> Self {
        Self {
            zone: value.0,
            label: heart_rate_zone_label(value.0),
            min_bpm: value.1,
            max_bpm: value.2,
            duration_seconds: value.3,
            share_percent: decode_share_percent(value.4),
        }
    }
}

pub fn serialize_activity_heart_rate_zones(
    zones: &[ActivityHeartRateZoneSummary],
) -> Option<StoredActivityHeartRateZones> {
    (!zones.is_empty()).then(|| {
        StoredActivityHeartRateZones(
            zones
                .iter()
                .map(StoredActivityHeartRateZoneSummary::from)
                .collect(),
        )
    })
}

pub fn deserialize_activity_heart_rate_zones(
    raw: Option<&StoredActivityHeartRateZones>,
) -> Vec<ActivityHeartRateZoneSummary> {
    raw.map(|value| {
        value
            .0
            .iter()
            .cloned()
            .map(ActivityHeartRateZoneSummary::from)
            .collect()
    })
    .unwrap_or_default()
}

pub fn weighted_zone_intensity(zones: &[ActivityHeartRateZoneSummary]) -> Option<f64> {
    let total_seconds: i32 = zones.iter().map(|zone| zone.duration_seconds.max(0)).sum();

    if total_seconds <= 0 {
        return None;
    }

    let weighted_sum = zones
        .iter()
        .map(|zone| zone_intensity(zone.zone) * f64::from(zone.duration_seconds.max(0)))
        .sum::<f64>();

    Some(weighted_sum / f64::from(total_seconds))
}

fn heart_rate_zone_label(zone: i32) -> String {
    HEART_RATE_ZONE_LABELS
        .get(zone.saturating_sub(1) as usize)
        .map(|value| (*value).to_string())
        .unwrap_or_else(|| format!("Z{zone}"))
}

fn encode_share_percent(value: f64) -> i32 {
    (value * HEART_RATE_ZONE_SHARE_PERCENT_SCALE).round() as i32
}

fn decode_share_percent(value: i32) -> f64 {
    f64::from(value) / HEART_RATE_ZONE_SHARE_PERCENT_SCALE
}

fn zone_intensity(zone: i32) -> f64 {
    match zone {
        1 => 0.55,
        2 => 0.65,
        3 => 0.75,
        4 => 0.85,
        _ => 0.95,
    }
}
