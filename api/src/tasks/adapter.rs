use sea_orm::DatabaseConnection;
use std::sync::Arc;

pub use kaleido::auth::{AuthTaskQueue as TaskQueue, DefaultEnvAuthService as AppAuthService};

pub fn create_auth_service(db: DatabaseConnection, tasks: TaskQueue) -> AppAuthService {
    let metrics = kaleido::auth::FnMetricsRecorder::new(
        || kaleido::glass::api_metrics::login_counter().inc(),
        || kaleido::glass::api_metrics::failed_login_counter().inc(),
        || kaleido::glass::api_metrics::logout_counter().inc(),
        || kaleido::glass::api_metrics::token_refresh_counter().inc(),
    );
    kaleido::auth::build_default_auth_service(
        Arc::new(db),
        kaleido::auth::AuthEmailService::new(tasks),
        kaleido::auth::EnvConfigProvider::from_env(),
        metrics,
    )
}
