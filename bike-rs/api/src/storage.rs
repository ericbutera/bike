use crate::tasks::TaskQueue;
use crate::tasks::{create_session_service, AppSessionService};
use bike_core::auth::controllers::oauth::OAuthRouteStorage;
use bike_core::auth::{AuthRouteStorage, AuthStorage};
use bike_core::config::Config;
use bike_core::platform::feature_flags::{FeatureFlagService, FeatureFlagStorage};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppStorage {
    pub strava_intake: integrations::strava::events::StravaIntake,
    pub task_submission: std::sync::Arc<dyn bike_temporal::submission::ReprocessSubmission>,
    pub map_images: Option<std::sync::Arc<bike_core::activity_maps::MapImageService>>,
    pub heatmaps: std::sync::Arc<bike_core::heatmaps::service::HeatmapService>,
    pub db: DatabaseConnection,
    pub tasks: TaskQueue,
    pub feature_flags: FeatureFlagService,
    pub session_service: AppSessionService,
    pub uploads_dir: String,
    pub local_admin_user_pid: Option<Uuid>,
}

impl AppStorage {
    #[cfg(test)]
    pub(crate) fn for_test(db: DatabaseConnection) -> Self {
        Self {
            strava_intake: integrations::strava::events::StravaIntake::new(db.clone()),
            task_submission: std::sync::Arc::new(bike_temporal::submission::TaskSubmissionService::from_env()),
            map_images: None,
            heatmaps: std::sync::Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
            tasks: TaskQueue::new(db.clone()),
            feature_flags: FeatureFlagService::new(),
            session_service: create_session_service(db.clone()),
            db,
            uploads_dir: "/tmp".into(),
            local_admin_user_pid: None,
        }
    }

    pub async fn new(database_url: &str) -> Self {
        let db = connect_database(database_url).await.expect("DB connection failed");

        let tasks = TaskQueue::new(db.clone());

        let feature_flags = FeatureFlagService::new();
        if let Err(err) = feature_flags.load_cache(&db).await {
            tracing::warn!(error = ?err, "failed to load feature flags cache");
        }

        let session_service = create_session_service(db.clone());

        let local_admin_user_pid = if Config::get().local_admin_enabled {
            Some(ensure_local_admin(&db).await)
        } else {
            None
        };

        let uploads_dir = Config::get().uploads_dir.clone();
        std::fs::create_dir_all(&uploads_dir).expect("Failed to create uploads directory");

        Self {
            strava_intake: integrations::strava::events::StravaIntake::new(db.clone()),
            task_submission: std::sync::Arc::new(bike_temporal::submission::TaskSubmissionService::from_env()),
            map_images: create_map_image_service(Config::get()).await,
            heatmaps: std::sync::Arc::new(bike_core::heatmaps::service::HeatmapService::default()),
            db,
            tasks,
            feature_flags,
            session_service,
            uploads_dir,
            local_admin_user_pid,
        }
    }
}

async fn create_map_image_service(
    config: &Config,
) -> Option<std::sync::Arc<bike_core::activity_maps::MapImageService>> {
    let url = config.map_renderer_grpc_address.as_ref()?;
    let service = std::sync::Arc::new(
        bike_core::activity_maps::MapImageService::new(
            url.clone(),
            config.map_service_token.clone(),
            config.map_image_cache_dir.clone().into(),
            std::time::Duration::from_secs(config.map_image_cache_ttl_seconds),
        )
        .await
        .expect("Failed to initialize map image cache"),
    );
    service
        .prune()
        .await
        .expect("Failed to prune map image cache at startup");
    service.maintain();
    Some(service)
}

async fn ensure_local_admin(db: &DatabaseConnection) -> Uuid {
    let user = bike_core::auth::OAuthService::find_or_create_provider_user(
        db,
        bike_core::auth::PROVIDER_DEV,
        bike_core::auth::OAuthService::local_dev_user_info(),
    )
    .await
    .expect("failed to initialize the local development admin user");

    user.ensure_admin(db)
        .await
        .expect("failed to grant local development admin access");

    user.pid
}

pub use bike_core::db::connect_database;

impl FeatureFlagStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn feature_flag_service(&self) -> &FeatureFlagService {
        &self.feature_flags
    }
}

impl AuthStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn jwt_secret(&self) -> &str {
        &Config::get().jwt_secret
    }

    fn local_admin_user_pid(&self) -> Option<Uuid> {
        self.local_admin_user_pid
    }
}

impl AuthRouteStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn session_service(&self) -> &AppSessionService {
        &self.session_service
    }

    fn frontend_url(&self) -> &str {
        &Config::get().frontend_url
    }
}

impl OAuthRouteStorage for AppStorage {
    fn api_url(&self) -> &str {
        &Config::get().api_url
    }

    fn oauth_enabled(&self) -> bool {
        bike_core::auth::OAuthProviderService::is_any_provider_enabled()
    }

    fn oauth_provider_enabled(&self, provider: &str) -> bool {
        bike_core::auth::OAuthProviderService::is_provider_enabled(provider)
    }
}
