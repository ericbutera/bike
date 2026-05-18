use crate::activity_details::{ActivityChartPoint, ActivityRoutePoint};
use crate::app_error::AppError;
use crate::entities::user_preferences;
use sea_orm::FromJsonQueryResult;
use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const HEART_RATE_ZONE_BOUNDARY_COUNT: usize = 4;
const HEART_RATE_ZONE_LABELS: [&str; 5] = ["Z1", "Z2", "Z3", "Z4", "Z5"];
const MIN_HEART_RATE_BPM: i32 = 40;
const MAX_HEART_RATE_BPM: i32 = 240;
const MIN_ESTIMATED_FTP_WATTS: i32 = 80;
const MAX_ESTIMATED_FTP_WATTS: i32 = 600;
const HEART_RATE_ZONE_SHARE_PERCENT_SCALE: f64 = 1000.0;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrainingProfile {
    pub estimated_ftp_watts: Option<i32>,
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
}

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

#[derive(Debug, Clone, Copy)]
struct HeartRateSample {
    elapsed_seconds: i32,
    heart_rate_bpm: Option<i32>,
}

pub async fn load_training_profile<C>(db: &C, user_id: i32) -> Result<TrainingProfile, AppError>
where
    C: ConnectionTrait,
{
    let model = user_preferences::Entity::find()
        .filter(user_preferences::Column::UserId.eq(user_id))
        .one(db)
        .await?;

    Ok(training_profile_from_preferences(model.as_ref()))
}

pub fn training_profile_from_preferences(
    model: Option<&user_preferences::Model>,
) -> TrainingProfile {
    TrainingProfile {
        estimated_ftp_watts: model.and_then(|preferences| preferences.estimated_ftp_watts),
        heart_rate_zone_bounds_bpm: model.and_then(|preferences| {
            deserialize_heart_rate_zone_bounds(preferences.heart_rate_zone_bounds_json.as_ref())
        }),
    }
}

pub fn validate_estimated_ftp_watts(value: Option<i32>) -> Result<Option<i32>, AppError> {
    match value {
        Some(ftp) if !(MIN_ESTIMATED_FTP_WATTS..=MAX_ESTIMATED_FTP_WATTS).contains(&ftp) => {
            Err(AppError::validation_field(
                "estimated_ftp_watts",
                "Estimated FTP must be between 80 and 600 watts",
            ))
        }
        _ => Ok(value),
    }
}

pub fn validate_heart_rate_zone_bounds_bpm(
    value: Option<Vec<i32>>,
) -> Result<Option<Vec<i32>>, AppError> {
    let Some(bounds) = value else {
        return Ok(None);
    };

    if bounds.is_empty() {
        return Ok(None);
    }

    if bounds.len() != HEART_RATE_ZONE_BOUNDARY_COUNT {
        return Err(AppError::validation_field(
            "heart_rate_zone_bounds_bpm",
            "Heart rate zones must include exactly four ascending zone ceilings",
        ));
    }

    let mut previous = None;
    for bound in &bounds {
        if !(MIN_HEART_RATE_BPM..=MAX_HEART_RATE_BPM).contains(bound) {
            return Err(AppError::validation_field(
                "heart_rate_zone_bounds_bpm",
                "Heart rate zone ceilings must be between 40 and 240 bpm",
            ));
        }

        if let Some(previous_bound) = previous {
            if *bound <= previous_bound {
                return Err(AppError::validation_field(
                    "heart_rate_zone_bounds_bpm",
                    "Heart rate zone ceilings must be strictly increasing",
                ));
            }
        }

        previous = Some(*bound);
    }

    Ok(Some(bounds))
}

pub fn serialize_heart_rate_zone_bounds(
    bounds: Option<&[i32]>,
) -> Result<Option<StoredHeartRateZoneBounds>, AppError> {
    match bounds {
        Some(values) if !values.is_empty() => Ok(Some(StoredHeartRateZoneBounds(values.to_vec()))),
        _ => Ok(None),
    }
}

pub fn deserialize_heart_rate_zone_bounds(raw: Option<&StoredHeartRateZoneBounds>) -> Option<Vec<i32>> {
    raw.map(|value| value.0.clone()).filter(|value| !value.is_empty())
}

pub fn serialize_activity_heart_rate_zones(
    zones: &[ActivityHeartRateZoneSummary],
) -> Result<Option<StoredActivityHeartRateZones>, AppError> {
    if zones.is_empty() {
        return Ok(None);
    }

    Ok(Some(StoredActivityHeartRateZones(
        zones
            .iter()
            .map(StoredActivityHeartRateZoneSummary::from)
            .collect(),
    )))
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

pub fn summarize_heart_rate_zones(
    route_points: &[ActivityRoutePoint],
    chart_points: &[ActivityChartPoint],
    fallback_duration_seconds: Option<i32>,
    fallback_average_heart_rate_bpm: Option<i32>,
    heart_rate_zone_bounds_bpm: Option<&[i32]>,
) -> Vec<ActivityHeartRateZoneSummary> {
    let Some(bounds) = heart_rate_zone_bounds_bpm else {
        return Vec::new();
    };

    let route_samples = route_points
        .iter()
        .map(|point| HeartRateSample {
            elapsed_seconds: point.elapsed_seconds,
            heart_rate_bpm: point.heart_rate_bpm,
        })
        .collect::<Vec<_>>();
    let chart_samples = chart_points
        .iter()
        .map(|point| HeartRateSample {
            elapsed_seconds: point.elapsed_seconds,
            heart_rate_bpm: point.heart_rate_bpm,
        })
        .collect::<Vec<_>>();

    let durations = summarize_zone_durations(&route_samples, bounds)
        .or_else(|| summarize_zone_durations(&chart_samples, bounds))
        .unwrap_or_else(|| {
            let mut durations = vec![0; HEART_RATE_ZONE_LABELS.len()];
            if let (Some(duration_seconds), Some(average_heart_rate_bpm)) = (
                fallback_duration_seconds.filter(|value| *value > 0),
                fallback_average_heart_rate_bpm,
            ) {
                let index = zone_index_for_heart_rate(average_heart_rate_bpm, bounds);
                durations[index] = duration_seconds;
            }
            durations
        });

    let total_seconds: i32 = durations.iter().sum();

    HEART_RATE_ZONE_LABELS
        .iter()
        .enumerate()
        .map(|(index, label)| {
            let min_bpm = if index == 0 {
                None
            } else {
                Some(bounds[index - 1] + 1)
            };
            let max_bpm = if index < bounds.len() {
                Some(bounds[index])
            } else {
                None
            };
            let duration_seconds = durations[index];

            ActivityHeartRateZoneSummary {
                zone: (index + 1) as i32,
                label: (*label).to_string(),
                min_bpm,
                max_bpm,
                duration_seconds,
                share_percent: if total_seconds > 0 {
                    round_metric(duration_seconds as f64 / f64::from(total_seconds) * 100.0)
                } else {
                    0.0
                },
            }
        })
        .collect()
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

fn summarize_zone_durations(samples: &[HeartRateSample], bounds: &[i32]) -> Option<Vec<i32>> {
    if samples.len() < 2 {
        return None;
    }

    let mut durations = vec![0; HEART_RATE_ZONE_LABELS.len()];
    let mut total_seconds = 0;

    for window in samples.windows(2) {
        let start = window[0];
        let end = window[1];
        let delta_seconds = end.elapsed_seconds - start.elapsed_seconds;
        if delta_seconds <= 0 {
            continue;
        }

        let Some(heart_rate_bpm) = start.heart_rate_bpm.or(end.heart_rate_bpm) else {
            continue;
        };

        let index = zone_index_for_heart_rate(heart_rate_bpm, bounds);
        durations[index] += delta_seconds;
        total_seconds += delta_seconds;
    }

    if total_seconds > 0 {
        Some(durations)
    } else {
        None
    }
}

fn zone_index_for_heart_rate(heart_rate_bpm: i32, bounds: &[i32]) -> usize {
    bounds
        .iter()
        .position(|bound| heart_rate_bpm <= *bound)
        .unwrap_or(bounds.len())
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

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_heart_rate_zone_bounds_accepts_four_ascending_values() {
        assert_eq!(
            validate_heart_rate_zone_bounds_bpm(Some(vec![120, 140, 155, 170])).unwrap(),
            Some(vec![120, 140, 155, 170])
        );
    }

    #[test]
    fn validate_heart_rate_zone_bounds_rejects_invalid_shape() {
        let error =
            validate_heart_rate_zone_bounds_bpm(Some(vec![120, 120, 155, 170])).unwrap_err();

        assert_eq!(
            error.message,
            "Heart rate zone ceilings must be strictly increasing"
        );
    }

    #[test]
    fn summarize_heart_rate_zones_uses_track_samples() {
        let zones = summarize_heart_rate_zones(
            &[
                ActivityRoutePoint {
                    elapsed_seconds: 0,
                    latitude: 45.0,
                    longitude: -122.0,
                    distance_meters: None,
                    elevation_meters: None,
                    speed_mps: None,
                    heart_rate_bpm: Some(118),
                    cadence_rpm: None,
                    power_watts: None,
                },
                ActivityRoutePoint {
                    elapsed_seconds: 300,
                    latitude: 45.1,
                    longitude: -122.1,
                    distance_meters: None,
                    elevation_meters: None,
                    speed_mps: None,
                    heart_rate_bpm: Some(148),
                    cadence_rpm: None,
                    power_watts: None,
                },
                ActivityRoutePoint {
                    elapsed_seconds: 600,
                    latitude: 45.2,
                    longitude: -122.2,
                    distance_meters: None,
                    elevation_meters: None,
                    speed_mps: None,
                    heart_rate_bpm: Some(176),
                    cadence_rpm: None,
                    power_watts: None,
                },
            ],
            &[],
            None,
            None,
            Some(&[120, 140, 155, 170]),
        );

        assert_eq!(zones[0].duration_seconds, 300);
        assert_eq!(zones[2].duration_seconds, 300);
        assert_eq!(zones[4].duration_seconds, 0);
        assert_eq!(zones[0].share_percent, 50.0);
        assert_eq!(zones[2].share_percent, 50.0);
    }

    #[test]
    fn weighted_zone_intensity_averages_zone_distribution() {
        let intensity = weighted_zone_intensity(&[
            ActivityHeartRateZoneSummary {
                zone: 2,
                label: "Z2".to_string(),
                min_bpm: Some(121),
                max_bpm: Some(140),
                duration_seconds: 1800,
                share_percent: 50.0,
            },
            ActivityHeartRateZoneSummary {
                zone: 4,
                label: "Z4".to_string(),
                min_bpm: Some(156),
                max_bpm: Some(170),
                duration_seconds: 1800,
                share_percent: 50.0,
            },
        ])
        .unwrap();

        assert_eq!(round_metric(intensity), 0.8);
    }

    #[test]
    fn serializes_activity_heart_rate_zones_to_compact_arrays() {
        let stored = serialize_activity_heart_rate_zones(&[ActivityHeartRateZoneSummary {
            zone: 3,
            label: "Z3".to_string(),
            min_bpm: Some(141),
            max_bpm: Some(155),
            duration_seconds: 840,
            share_percent: 33.3,
        }])
        .expect("stored zones")
        .expect("zones payload");

        assert_eq!(serde_json::to_value(stored).unwrap(), json!([[3, 141, 155, 840, 33300]]));
    }

    #[test]
    fn serializes_heart_rate_zone_bounds_to_json_array() {
        let stored = serialize_heart_rate_zone_bounds(Some(&[120, 140, 155, 170]))
            .expect("stored bounds")
            .expect("bounds payload");

        assert_eq!(serde_json::to_value(stored).unwrap(), json!([120, 140, 155, 170]));
    }
}
