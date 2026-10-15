//! Insert fixtures with deterministic timestamps and database-owned defaults.
use super::timestamp;
use crate::{
    background_jobs::background_tasks, entities::user_preferences,
    platform::feature_flags::entities as feature_flags,
};
use sea_orm::Set;

pub fn new_background_task() -> background_tasks::ActiveModel {
    background_tasks::ActiveModel {
        task_type: Set("fixture".into()),
        payload: Set(serde_json::json!({})),
        created_at: Set(timestamp()),
        updated_at: Set(timestamp()),
        ..Default::default()
    }
}

pub fn new_user_preferences() -> user_preferences::ActiveModel {
    user_preferences::ActiveModel {
        user_id: Set(super::customer().id),
        created_at: Set(timestamp()),
        updated_at: Set(timestamp()),
        ..Default::default()
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
