use crate::tasks::TaskQueue;
use crate::tasks::{create_auth_service, AppAuthService};
use bike_core::config::Config;
use kaleido::auth::controllers::oauth::OAuthRouteStorage;
use kaleido::auth::{AuthRouteStorage, AuthStorage};
use kaleido::background_jobs::admin::BackgroundTasksStorage;
use kaleido::glass::feature_flags::{FeatureFlagService, FeatureFlagStorage};
use kaleido::glass::metrics_controller::MetricsStorage;
use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppStorage {
    pub db: DatabaseConnection,
    pub tasks: TaskQueue,
    pub feature_flags: FeatureFlagService,
    pub auth_service: AppAuthService,
    pub uploads_dir: String,
    pub local_admin_user_pid: Option<Uuid>,
}

impl AppStorage {
    pub async fn new(database_url: &str) -> Self {
        let db = connect_database(database_url)
            .await
            .expect("DB connection failed");

        let tasks = TaskQueue::new(db.clone());

        let feature_flags = FeatureFlagService::new();
        if let Err(err) = feature_flags.load_cache(&db).await {
            tracing::warn!(error = ?err, "failed to load feature flags cache");
        }

        let auth_service = create_auth_service(db.clone(), tasks.clone());

        let local_admin_user_pid = if Config::get().local_admin_enabled {
            Some(ensure_local_admin(&db).await)
        } else {
            None
        };

        let uploads_dir = Config::get().uploads_dir.clone();
        std::fs::create_dir_all(&uploads_dir).expect("Failed to create uploads directory");

        Self {
            db,
            tasks,
            feature_flags,
            auth_service,
            uploads_dir,
            local_admin_user_pid,
        }
    }
}

async fn ensure_local_admin(db: &DatabaseConnection) -> Uuid {
    let user = kaleido::auth::OAuthService::find_or_create_provider_user(
        db,
        kaleido::auth::PROVIDER_DEV,
        kaleido::auth::OAuthService::local_dev_user_info(),
    )
    .await
    .expect("failed to initialize the local development admin user");

    if user.is_admin != Some(true) {
        let mut active_user: kaleido::auth::entities::users::ActiveModel = user.clone().into();
        active_user.is_admin = Set(Some(true));
        active_user
            .update(db)
            .await
            .expect("failed to grant local development admin access");
    }

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

    fn local_admin_user_pid(&self) -> Option<Uuid> {
        self.local_admin_user_pid
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
