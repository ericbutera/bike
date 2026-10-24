use super::{climb_from_detection, Climb};
use crate::{activity_data::ActivityRoutePoint, services::reports::ClimbResponse};
use serde::{Deserialize, Serialize};

/// Immutable climb artifact, including facts prepared for the later report read cutover.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PreparedOutput {
    pub detector_version: i32,
    pub recording_sha256: String,
    pub climbs: Vec<PreparedClimb>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct PreparedClimb {
    #[serde(flatten)]
    pub climb: Climb,
    #[serde(flatten)]
    pub facts: ReportFacts,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct ReportFacts {
    pub vertical_rate_meters_per_hour: f64,
    pub average_speed_mps: Option<f64>,
    pub average_heart_rate_bpm: Option<f64>,
    pub peak_heart_rate_bpm: Option<i32>,
    pub average_cadence_rpm: Option<f64>,
    pub average_power_watts: Option<f64>,
    pub heart_rate_recovery_30_seconds_bpm: Option<i32>,
    pub heart_rate_recovery_60_seconds_bpm: Option<i32>,
    pub seconds_to_drop_10_bpm: Option<i32>,
    pub seconds_to_drop_15_bpm: Option<i32>,
    pub summit_immediately_enters_descent: bool,
    pub is_first_half: bool,
}

impl PreparedClimb {
    pub fn from_detection(report: ClimbResponse, route: &[ActivityRoutePoint]) -> Self {
        Self {
            climb: climb_from_detection(&report, route),
            facts: ReportFacts {
                vertical_rate_meters_per_hour: report.vertical_rate_meters_per_hour,
                average_speed_mps: report.average_speed_mps,
                average_heart_rate_bpm: report.average_heart_rate_bpm,
                peak_heart_rate_bpm: report.peak_heart_rate_bpm,
                average_cadence_rpm: report.average_cadence_rpm,
                average_power_watts: report.average_power_watts,
                heart_rate_recovery_30_seconds_bpm: report.heart_rate_recovery_30_seconds_bpm,
                heart_rate_recovery_60_seconds_bpm: report.heart_rate_recovery_60_seconds_bpm,
                seconds_to_drop_10_bpm: report.seconds_to_drop_10_bpm,
                seconds_to_drop_15_bpm: report.seconds_to_drop_15_bpm,
                summit_immediately_enters_descent: report.summit_immediately_enters_descent,
                is_first_half: report.first_or_second_half == "first",
            },
        }
    }
}
