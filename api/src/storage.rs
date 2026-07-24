use crate::config::Config;
use crate::tasks::TaskQueue;
use crate::tasks::{create_auth_service, AppAuthService};
use kaleido::auth::controllers::oauth::OAuthRouteStorage;
use kaleido::auth::{AuthRouteStorage, AuthStorage};
use kaleido::background_jobs::admin::BackgroundTasksStorage;
use kaleido::glass::feature_flags::{FeatureFlagService, FeatureFlagStorage};
use kaleido::glass::metrics_controller::MetricsStorage;
use migration::MigratorTrait;
use sea_orm::DatabaseConnection;

#[derive(Clone)]
pub struct AppStorage {
    pub db: DatabaseConnection,
    pub tasks: TaskQueue,
    pub feature_flags: FeatureFlagService,
    pub auth_service: AppAuthService,
    pub uploads_dir: String,
}

impl AppStorage {
    pub async fn new(database_url: &str) -> Self {
        let db = sea_orm::Database::connect(database_url)
            .await
            .expect("DB connection failed");

        migration::Migrator::up(&db, None)
            .await
            .expect("Migration failed");

        let tasks = TaskQueue::new(db.clone());

        let feature_flags = FeatureFlagService::new();
        if let Err(err) = feature_flags.load_cache(&db).await {
            tracing::warn!(error = ?err, "failed to load feature flags cache");
        }

        let auth_service = create_auth_service(db.clone(), tasks.clone());

        let uploads_dir = Config::get().uploads_dir.clone();
        std::fs::create_dir_all(&uploads_dir).expect("Failed to create uploads directory");

        Self {
            db,
            tasks,
            feature_flags,
            auth_service,
            uploads_dir,
        }
    }
}

impl FeatureFlagStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn feature_flag_service(&self) -> &FeatureFlagService {
        &self.feature_flags
    }
}

impl BackgroundTasksStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}

impl AuthStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn jwt_secret(&self) -> &str {
        &Config::get().jwt_secret
    }
}

impl AuthRouteStorage for AppStorage {
    type EmailService = kaleido::auth::AuthEmailService;
    type CooldownManager = kaleido::auth::DefaultCooldownManager;
    type AuditLogger = kaleido::auth::SeaOrmAuditLogger;
    type MetricsRecorder = kaleido::auth::FnMetricsRecorder;
    type ConfigProvider = kaleido::auth::EnvConfigProvider;

    fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    fn auth_service(&self) -> &AppAuthService {
        &self.auth_service
    }

    fn frontend_url(&self) -> &str {
        &Config::get().frontend_url
    }

    fn password_auth_enabled(&self) -> bool {
        Config::get().auth_password_enabled
    }

    fn registration_enabled(&self) -> bool {
        Config::get().auth_registration_enabled
    }
}

impl OAuthRouteStorage for AppStorage {
    fn api_url(&self) -> &str {
        &Config::get().api_url
    }

    fn oauth_enabled(&self) -> bool {
        kaleido::auth::OAuthProviderService::is_any_provider_enabled()
    }

    fn oauth_provider_enabled(&self, provider: &str) -> bool {
        kaleido::auth::OAuthProviderService::is_provider_enabled(provider)
    }
}

impl MetricsStorage for AppStorage {
    fn db(&self) -> &DatabaseConnection {
        &self.db
    }
}
