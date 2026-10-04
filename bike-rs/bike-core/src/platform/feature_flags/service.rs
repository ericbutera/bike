use super::entities as feature_flags;
use crate::platform::error::PlatformError;
use open_feature::OpenFeature;
use sea_orm::{prelude::*, ActiveValue, QueryOrder};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Service for managing feature flags with in-memory caching
#[derive(Clone)]
pub struct FeatureFlagService {
    cache: Arc<RwLock<HashMap<String, bool>>>,
}

impl FeatureFlagService {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Load all feature flags from database into cache
    pub async fn load_cache(&self, db: &DatabaseConnection) -> Result<(), PlatformError> {
        let flags = feature_flags::Entity::find()
            .order_by_asc(feature_flags::Column::FeatureKey)
            .all(db)
            .await?;

        let mut evaluated = HashMap::new();

        // Prefer provider evaluation when available. Fall back to DB value on error.
        let api = OpenFeature::singleton().await;
        let client = api.create_client();

        for flag in flags {
            let provider_val = client
                .get_bool_value(&flag.feature_key, None, None)
                .await
                .ok();

            let final_val = provider_val.unwrap_or(flag.enabled);
            evaluated.insert(flag.feature_key.clone(), final_val);
        }
        *self.cache.write().unwrap() = evaluated;
        Ok(())
    }

    /// Check if a feature is enabled (uses cache)
    pub fn is_enabled(&self, feature_key: &str) -> bool {
        let cache = self.cache.read().unwrap();
        cache.get(feature_key).copied().unwrap_or(false)
    }

    /// List all feature flags from database
    pub async fn list_all(
        db: &DatabaseConnection,
    ) -> Result<Vec<feature_flags::Model>, PlatformError> {
        let flags = feature_flags::Entity::find()
            .order_by_asc(feature_flags::Column::FeatureKey)
            .all(db)
            .await?;
        Ok(flags)
    }

    /// Get a specific feature flag by key
    pub async fn get_by_key(
        db: &DatabaseConnection,
        feature_key: &str,
    ) -> Result<Option<feature_flags::Model>, PlatformError> {
        let flag = feature_flags::Entity::find()
            .filter(feature_flags::Column::FeatureKey.eq(feature_key))
            .one(db)
            .await?;
        Ok(flag)
    }

    /// Update a feature flag's enabled status
    pub async fn update_flag(
        &self,
        db: &DatabaseConnection,
        feature_key: &str,
        enabled: bool,
    ) -> Result<feature_flags::Model, PlatformError> {
        let flag = Self::get_by_key(db, feature_key)
            .await?
            .ok_or_else(|| PlatformError::entity_not_found("Feature flag not found"))?;

        let mut active_model: feature_flags::ActiveModel = flag.into();
        active_model.enabled = ActiveValue::Set(enabled);
        active_model.updated_at = ActiveValue::Set(chrono::Utc::now());

        let updated = active_model.update(db).await?;

        // Update cache
        let mut cache = self.cache.write().unwrap();
        cache.insert(feature_key.to_string(), enabled);

        Ok(updated)
    }
}

impl Default for FeatureFlagService {
    fn default() -> Self {
        Self::new()
    }
}
