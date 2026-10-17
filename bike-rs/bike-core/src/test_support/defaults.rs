//! Insert fixtures with deterministic timestamps and database-owned defaults.
use super::timestamp;
use crate::{entities::user_preferences, platform::feature_flags::entities as feature_flags};
use sea_orm::Set;

pub fn new_user_preferences() -> user_preferences::ActiveModel {
    user_preferences::ActiveModel {
        user_id: Set(super::customer().id),
        created_at: Set(timestamp()),
        updated_at: Set(timestamp()),
        ..Default::default()
    }
}

pub fn user_preferences() -> user_preferences::Model {
    user_preferences::Model {
        id: 1,
        user_id: super::customer().id,
        unit_system: "imperial".into(),
        estimated_ftp_watts: None,
        heart_rate_zone_bounds_json: None,
        xc_goal_start_date: None,
        xc_goal_target_date: None,
        xc_goal_target_distance_meters: None,
        xc_goal_target_elevation_gain_meters: None,
        xc_goal_event_name: None,
        xc_goal_target_finish_time_seconds: None,
        xc_goal_event_profile: None,
        xc_goal_backfill_status: None,
        xc_goal_backfill_completed_at: None,
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

pub fn new_feature_flag() -> feature_flags::ActiveModel {
    feature_flags::ActiveModel {
        feature_key: Set("fixture".into()),
        created_at: Set(timestamp()),
        updated_at: Set(timestamp()),
        ..Default::default()
    }
}
