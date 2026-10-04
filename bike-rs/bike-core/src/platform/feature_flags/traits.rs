use sea_orm::DatabaseConnection;

use super::service::FeatureFlagService;

/// Generic storage trait for feature flag routes
pub trait FeatureFlagStorage: Send + Sync + 'static {
    fn db(&self) -> &DatabaseConnection;
    fn feature_flag_service(&self) -> &FeatureFlagService;
}
