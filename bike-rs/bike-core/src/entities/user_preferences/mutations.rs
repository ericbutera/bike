use super::{ActiveModel, Model};
use crate::training_profile::{
    serialize_heart_rate_zone_bounds, validate_estimated_ftp_watts, validate_heart_rate_zone_bounds_bpm,
};
use crate::workflow_error::WorkflowError;
use chrono::{DateTime, NaiveDate, Utc};
use sea_orm::{ActiveModelTrait, ConnectionTrait, DbErr, Set};

const XC_GOAL_EVENT_NAME_MAX_LENGTH: usize = 120;
const METERS_PER_MILE: f64 = 1609.344;
const METERS_PER_FOOT: f64 = 0.3048;
const XC_GOAL_TARGET_DISTANCE_MAX_METERS: f64 = 500.0 * METERS_PER_MILE;
const XC_GOAL_TARGET_ELEVATION_GAIN_MAX_METERS: f64 = 25_000.0 * METERS_PER_FOOT;
const XC_GOAL_EVENT_PROFILES: &[&str] = &[
    "xc_marathon",
    "technical_singletrack",
    "endurance_mtb",
    "ultra_mtb",
    "custom",
];

#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
#[schema(as = UpdateUserPreferencesRequest, example = json!({"unit_system": "metric","estimated_ftp_watts": 245,"heart_rate_zone_bounds_bpm": [110,130,150,170],"xc_goal_event_name": "Autumn 100 km","xc_goal_event_profile": "xc_marathon","xc_goal_start_date": "2026-09-01","xc_goal_target_date": "2026-10-31","xc_goal_target_distance_meters": 100000.0,"xc_goal_target_elevation_gain_meters": 1500.0,"xc_goal_target_finish_time_seconds": 21600}))]
pub struct PreferencesUpdate {
    #[schema(example = "metric")]
    pub unit_system: String,
    #[schema(example = 245)]
    pub estimated_ftp_watts: Option<i32>,
    #[schema(example = json!([110,130,150,170]))]
    pub heart_rate_zone_bounds_bpm: Option<Vec<i32>>,
    #[schema(example = "2026-09-01")]
    pub xc_goal_start_date: Option<String>,
    #[schema(example = "2026-10-31")]
    pub xc_goal_target_date: Option<String>,
    #[schema(example = 100000.0)]
    pub xc_goal_target_distance_meters: Option<f64>,
    #[schema(example = 1500.0)]
    pub xc_goal_target_elevation_gain_meters: Option<f64>,
    #[schema(example = "Autumn 100 km")]
    pub xc_goal_event_name: Option<String>,
    #[schema(example = 21600)]
    pub xc_goal_target_finish_time_seconds: Option<i32>,
    #[schema(example = "xc_marathon")]
    pub xc_goal_event_profile: Option<String>,
}

struct ValidatedPreferences {
    unit_system: String,
    estimated_ftp_watts: Option<i32>,
    heart_rate_zone_bounds_json: Option<crate::training_data::StoredHeartRateZoneBounds>,
    goal: XcGoalFields,
}

impl PreferencesUpdate {
    fn validate(self) -> Result<ValidatedPreferences, WorkflowError> {
        let unit_system = validate_unit_system(&self.unit_system)?;
        let estimated_ftp_watts = validate_estimated_ftp_watts(self.estimated_ftp_watts)?;
        let zones = validate_heart_rate_zone_bounds_bpm(self.heart_rate_zone_bounds_bpm)?;
        let goal = validate_xc_goal(
            self.xc_goal_start_date.as_deref(),
            self.xc_goal_target_date.as_deref(),
            self.xc_goal_target_distance_meters,
            self.xc_goal_target_elevation_gain_meters,
            self.xc_goal_event_name.as_deref(),
            self.xc_goal_target_finish_time_seconds,
            self.xc_goal_event_profile.as_deref(),
        )?;
        Ok(ValidatedPreferences {
            unit_system,
            estimated_ftp_watts,
            heart_rate_zone_bounds_json: serialize_heart_rate_zone_bounds(zones.as_deref())?,
            goal,
        })
    }
}

impl ValidatedPreferences {
    fn apply(self, active: &mut ActiveModel) {
        active.unit_system = Set(self.unit_system);
        active.estimated_ftp_watts = Set(self.estimated_ftp_watts);
        active.heart_rate_zone_bounds_json = Set(self.heart_rate_zone_bounds_json);
        active.xc_goal_start_date = Set(self.goal.start_date);
        active.xc_goal_target_date = Set(self.goal.target_date);
        active.xc_goal_target_distance_meters = Set(self.goal.target_distance_meters);
        active.xc_goal_target_elevation_gain_meters = Set(self.goal.target_elevation_gain_meters);
        active.xc_goal_event_name = Set(self.goal.event_name);
        active.xc_goal_target_finish_time_seconds = Set(self.goal.target_finish_time_seconds);
        active.xc_goal_event_profile = Set(self.goal.event_profile);
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum GoalBackfillChange {
    Queue,
    Clear,
    Unchanged,
}

#[derive(Debug)]
pub struct SavedPreferences {
    pub model: Model,
    pub goal_backfill: GoalBackfillChange,
}

impl Model {
    pub async fn save_preferences(
        db: &impl ConnectionTrait,
        user_id: i32,
        update: PreferencesUpdate,
    ) -> Result<SavedPreferences, WorkflowError> {
        let values = update.validate()?;
        let existing = Self::for_user(db, user_id).await?;
        let previous_start = existing.as_ref().and_then(|model| model.xc_goal_start_date);
        let goal_backfill = match values.goal.start_date {
            Some(start) if previous_start != Some(start) => GoalBackfillChange::Queue,
            None => GoalBackfillChange::Clear,
            _ => GoalBackfillChange::Unchanged,
        };
        let update_existing = existing.is_some();
        let clear_backfill = !update_existing || values.goal.start_date.is_none();
        let mut active = existing.map(ActiveModel::from).unwrap_or_default();
        active.user_id = Set(user_id);
        values.apply(&mut active);
        if clear_backfill {
            active.xc_goal_backfill_status = Set(None);
            active.xc_goal_backfill_completed_at = Set(None);
        }
        let model = if update_existing {
            active.update(db).await?
        } else {
            active.insert(db).await?
        };
        Ok(SavedPreferences { model, goal_backfill })
    }

    pub async fn set_goal_backfill(
        self,
        db: &impl ConnectionTrait,
        status: Option<&str>,
        completed_at: Option<DateTime<Utc>>,
    ) -> Result<(), DbErr> {
        let mut active: ActiveModel = self.into();
        active.xc_goal_backfill_status = Set(status.map(str::to_owned));
        active.xc_goal_backfill_completed_at = Set(completed_at);
        active.update(db).await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
struct XcGoalFields {
    start_date: Option<NaiveDate>,
    target_date: Option<NaiveDate>,
    target_distance_meters: Option<f64>,
    target_elevation_gain_meters: Option<f64>,
    event_name: Option<String>,
    target_finish_time_seconds: Option<i32>,
    event_profile: Option<String>,
}

fn validate_unit_system(value: &str) -> Result<String, WorkflowError> {
    let normalized = value.trim().to_ascii_lowercase();

    match normalized.as_str() {
        "metric" | "imperial" | "mixed" => Ok(normalized),
        _ => Err(WorkflowError::validation_field(
            "unit_system",
            "Unit system must be metric, imperial, or mixed",
        )),
    }
}

fn validate_xc_goal(
    start_date: Option<&str>,
    target_date: Option<&str>,
    target_distance_meters: Option<f64>,
    target_elevation_gain_meters: Option<f64>,
    event_name: Option<&str>,
    target_finish_time_seconds: Option<i32>,
    event_profile: Option<&str>,
) -> Result<XcGoalFields, WorkflowError> {
    let parsed_start_date = start_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_goal_date("xc_goal_start_date", "XC goal start date", value))
        .transpose()?;
    let parsed_target_date = target_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| parse_goal_date("xc_goal_target_date", "XC goal target date", value))
        .transpose()?;
    let target_distance_meters = validate_metric_range(
        "xc_goal_target_distance_meters",
        target_distance_meters,
        XC_GOAL_TARGET_DISTANCE_MAX_METERS,
        "XC goal target distance must be 500 miles or less",
    )?;
    let target_elevation_gain_meters = validate_metric_range(
        "xc_goal_target_elevation_gain_meters",
        target_elevation_gain_meters,
        XC_GOAL_TARGET_ELEVATION_GAIN_MAX_METERS,
        "XC goal climbing target must be 25,000 feet or less",
    )?;
    let event_name = validate_optional_event_name(event_name)?;
    let target_finish_time_seconds = validate_target_finish_time_seconds(target_finish_time_seconds)?;
    let event_profile = validate_event_profile(event_profile)?;

    let field_count = [
        parsed_start_date.is_some(),
        parsed_target_date.is_some(),
        target_distance_meters.is_some(),
        target_elevation_gain_meters.is_some(),
    ]
    .into_iter()
    .filter(|value| *value)
    .count();

    if field_count == 0 {
        let has_finish_details = target_finish_time_seconds.is_some() || event_profile.is_some();
        if event_name.is_some() || has_finish_details {
            return Err(WorkflowError::validation_field(
                "xc_goal_start_date",
                "XC goal event details require a complete event target",
            ));
        }

        return Ok(XcGoalFields::default());
    }

    if field_count != 4 {
        return Err(WorkflowError::validation_field(
            "xc_goal_start_date",
            "XC goal requires a training start date, target date, target distance, and target climbing gain",
        ));
    }

    if parsed_start_date > parsed_target_date {
        return Err(WorkflowError::validation_field(
            "xc_goal_start_date",
            "XC goal start date must be on or before the target date",
        ));
    }

    Ok(XcGoalFields {
        start_date: parsed_start_date,
        target_date: parsed_target_date,
        target_distance_meters,
        target_elevation_gain_meters,
        event_name,
        target_finish_time_seconds,
        event_profile,
    })
}

fn validate_optional_event_name(value: Option<&str>) -> Result<Option<String>, WorkflowError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };

    if value.chars().count() > XC_GOAL_EVENT_NAME_MAX_LENGTH {
        return Err(WorkflowError::validation_field(
            "xc_goal_event_name",
            "XC goal event name must be 120 characters or fewer",
        ));
    }

    Ok(Some(value.to_string()))
}

fn validate_target_finish_time_seconds(value: Option<i32>) -> Result<Option<i32>, WorkflowError> {
    match value {
        Some(seconds) if seconds <= 0 => Err(WorkflowError::validation_field(
            "xc_goal_target_finish_time_seconds",
            "Target finish time must be greater than zero",
        )),
        _ => Ok(value),
    }
}

fn validate_event_profile(value: Option<&str>) -> Result<Option<String>, WorkflowError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let normalized = value.to_ascii_lowercase();

    if !XC_GOAL_EVENT_PROFILES.contains(&normalized.as_str()) {
        return Err(WorkflowError::validation_field(
            "xc_goal_event_profile",
            "XC goal event profile is not supported",
        ));
    }

    Ok(Some(normalized))
}

fn validate_metric_range(
    field: &str,
    value: Option<f64>,
    max_value: f64,
    max_message: &str,
) -> Result<Option<f64>, WorkflowError> {
    match value {
        Some(next) if !next.is_finite() || next <= 0.0 => Err(WorkflowError::validation_field(
            field,
            "Value must be greater than zero",
        )),
        Some(next) if next > max_value => Err(WorkflowError::validation_field(field, max_message)),
        _ => Ok(value),
    }
}

fn parse_goal_date(field: &str, label: &str, value: &str) -> Result<NaiveDate, WorkflowError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| WorkflowError::validation_field(field, format!("{label} must use YYYY-MM-DD format")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    #[test]
    fn validate_unit_system_accepts_supported_values() {
        assert_eq!(validate_unit_system("metric").unwrap(), "metric");
        assert_eq!(validate_unit_system("imperial").unwrap(), "imperial");
        assert_eq!(validate_unit_system(" mixed ").unwrap(), "mixed");
    }
    #[test]
    fn validate_unit_system_rejects_unknown_values() {
        let error = validate_unit_system("nautical").unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "Unit system must be metric, imperial, or mixed");
    }
    #[test]
    fn validate_xc_goal_requires_complete_payload() {
        let error = validate_xc_goal(
            Some("2026-06-01"),
            Some("2026-09-20"),
            Some(100_000.0),
            None,
            None,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(
            error.message,
            "XC goal requires a training start date, target date, target distance, and target climbing gain"
        );
    }
    #[test]
    fn validate_xc_goal_rejects_invalid_date_format() {
        let error = validate_xc_goal(
            Some("2026-06-01"),
            Some("09/20/2026"),
            Some(100_000.0),
            Some(1_000.0),
            None,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "XC goal target date must use YYYY-MM-DD format");
    }
    #[test]
    fn validate_xc_goal_rejects_start_date_after_target_date() {
        let error = validate_xc_goal(
            Some("2026-09-21"),
            Some("2026-09-20"),
            Some(100_000.0),
            Some(1_000.0),
            None,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "XC goal start date must be on or before the target date");
    }
    #[test]
    fn validate_xc_goal_rejects_target_distance_above_guardrail() {
        let error = validate_xc_goal(
            Some("2026-06-01"),
            Some("2026-09-20"),
            Some(XC_GOAL_TARGET_DISTANCE_MAX_METERS + 1.0),
            Some(1_000.0),
            None,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "XC goal target distance must be 500 miles or less");
    }
    #[test]
    fn validate_xc_goal_rejects_target_climbing_above_guardrail() {
        let error = validate_xc_goal(
            Some("2026-06-01"),
            Some("2026-09-20"),
            Some(160_934.4),
            Some(130_000.0 * METERS_PER_FOOT),
            None,
            None,
            None,
        )
        .unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "XC goal climbing target must be 25,000 feet or less");
    }
    #[test]
    fn validate_xc_goal_accepts_optional_event_details() {
        let goal = validate_xc_goal(
            Some("2026-06-01"),
            Some("2026-09-20"),
            Some(160_934.4),
            Some(3_962.4),
            Some(" Marji Gesick MG100 "),
            Some(43_200),
            Some("TECHNICAL_SINGLETRACK"),
        )
        .unwrap();

        assert_eq!(goal.event_name.as_deref(), Some("Marji Gesick MG100"));
        assert_eq!(goal.target_finish_time_seconds, Some(43_200));
        assert_eq!(goal.event_profile.as_deref(), Some("technical_singletrack"));
    }
    #[test]
    fn validate_xc_goal_rejects_event_details_without_complete_target() {
        let error = validate_xc_goal(None, None, None, None, Some("Lumberjack 100"), Some(36_000), None).unwrap_err();

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.message, "XC goal event details require a complete event target");
    }
}
